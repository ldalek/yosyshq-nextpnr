use std::{collections::HashMap, ops::RangeBounds, sync::atomic::AtomicUsize};

use colored::Colorize;
use indicatif::{ParallelProgressIterator, ProgressBar, ProgressStyle};
use rayon::prelude::*;

use crate::{npnr, route::Arc};

pub enum Segment {
    Northeast,
    Southeast,
    Southwest,
    Northwest,
}

//        (x < P.x)
//            N
//            ^
//            |
// (y > P.y)  |  (y < P.y)
//     W <----P----> E
//            |
//            |
//            v
//            S
//        (x > P.x)
pub struct Coord {
    x: i32,
    y: i32,
}

impl Coord {
    pub fn new(x: i32, y: i32) -> Self {
        Self { x, y }
    }

    pub fn is_north_of(&self, other: &Self) -> bool {
        self.x < other.x
    }

    pub fn is_east_of(&self, other: &Self) -> bool {
        self.y < other.y
    }

    pub fn is_south_of(&self, other: &Self) -> bool {
        self.x > other.x
    }

    pub fn is_west_of(&self, other: &Self) -> bool {
        self.y > other.y
    }

    pub fn segment_from(&self, other: &Self) -> Segment {
        match (self.is_north_of(other), self.is_east_of(other)) {
            (true, true) => Segment::Northeast,
            (true, false) => Segment::Northwest,
            (false, true) => Segment::Southeast,
            (false, false) => Segment::Southwest,
        }
    }
}

impl From<npnr::Loc> for Coord {
    fn from(other: npnr::Loc) -> Self {
        Self {
            x: other.x,
            y: other.y,
        }
    }
}

pub fn find_partition_point(
    ctx: &npnr::Context,
    arcs: &[Arc],
    pips: &[npnr::PipId],
    x_start: i32,
    x_finish: i32,
    y_start: i32,
    y_finish: i32,
) -> (i32, i32, Vec<Arc>, Vec<Arc>, Vec<Arc>, Vec<Arc>) {
    let mut x = ((x_finish - x_start) / 2) + x_start;
    let mut y = ((y_finish - y_start) / 2) + y_start;
    let mut x_diff = (x_finish - x_start) / 4;
    let mut y_diff = (y_finish - y_start) / 4;

    let mut ne;
    let mut se;
    let mut sw;
    let mut nw;

    while x_diff != 0 {
        (ne, se, sw, nw) = partition(
            ctx,
            arcs,
            pips,
            x,
            y,
            x_start..=x_finish,
            y_start..=y_finish,
        );
        let north = ne.len() + nw.len();
        let south = se.len() + sw.len();

        let nets = (north + south) as f64;

        let ne_dist = f64::abs(((ne.len() as f64) / nets) - 0.25);
        let se_dist = f64::abs(((se.len() as f64) / nets) - 0.25);
        let sw_dist = f64::abs(((sw.len() as f64) / nets) - 0.25);
        let nw_dist = f64::abs(((nw.len() as f64) / nets) - 0.25);

        let distortion = 100.0 * (ne_dist + se_dist + sw_dist + nw_dist);

        // Stop early if Good Enough.
        if distortion <= 5.0 {
            return (x, y, ne, se, sw, nw);
        }

        x += match north.cmp(&south) {
            std::cmp::Ordering::Less => x_diff,
            std::cmp::Ordering::Equal => 0,
            std::cmp::Ordering::Greater => -x_diff,
        };

        let east = ne.len() + se.len();
        let west = nw.len() + sw.len();
        y += match east.cmp(&west) {
            std::cmp::Ordering::Less => y_diff,
            std::cmp::Ordering::Equal => 0,
            std::cmp::Ordering::Greater => -y_diff,
        };

        x_diff >>= 1;
        y_diff >>= 1;
    }

    (ne, se, sw, nw) = partition(
        ctx,
        arcs,
        pips,
        x,
        y,
        x_start..=x_finish,
        y_start..=y_finish,
    );

    let north = ne.len() + nw.len();
    let south = se.len() + sw.len();
    let nets = (north + south) as f64;

    let ne_dist = f64::abs(((ne.len() as f64) / nets) - 0.25);
    let se_dist = f64::abs(((se.len() as f64) / nets) - 0.25);
    let sw_dist = f64::abs(((sw.len() as f64) / nets) - 0.25);
    let nw_dist = f64::abs(((nw.len() as f64) / nets) - 0.25);

    log_info!(
        "Distortion: {:.02}%\n",
        100.0 * (ne_dist + se_dist + sw_dist + nw_dist)
    );

    (x, y, ne, se, sw, nw)
}

/// finds the y location a line would be split at if you split it at a certain x location
///
/// the function assumes the line goes on forever in both directions, and it truncates the actual coordinate
fn split_line_over_x(line: (npnr::Loc, npnr::Loc), x_location: i32) -> i32 {
    if line.0.x == line.1.x {
        // the line is a straight line in the direction, there is either infinite solutions, or none
        // we simply average the y coordinate to give a "best effort" guess
        return (line.0.y + line.1.y) / 2;
    }

    let x_diff = line.0.x - line.1.x;
    let y_diff = line.0.y - line.1.y;

    // i hope for no overflows, maybe promote to i64 to be sure?
    (y_diff * x_location + line.0.y * x_diff - line.0.x * y_diff) / x_diff
}

/// finds the x location a line would be split at if you split it at a certain y location, assuming the line goes on forever in both directions
fn split_line_over_y(line: (npnr::Loc, npnr::Loc), y_location: i32) -> i32 {
    // laziness supreme!
    split_line_over_x(
        (
            npnr::Loc {
                x: line.0.y,
                y: line.0.x,
                z: 0,
            },
            npnr::Loc {
                x: line.1.y,
                y: line.1.x,
                z: 0,
            },
        ),
        y_location,
    )
}

// A big thank you to @Spacecat-chan for fixing my broken and buggy partition code.
fn partition<R: RangeBounds<i32>>(
    ctx: &npnr::Context,
    arcs: &[Arc],
    pips: &[npnr::PipId],
    x: i32,
    y: i32,
    x_bounds: R,
    y_bounds: R,
) -> (Vec<Arc>, Vec<Arc>, Vec<Arc>, Vec<Arc>) {
    let mut pips_n = HashMap::new();
    let mut pips_e = HashMap::new();
    let mut pips_s = HashMap::new();
    let mut pips_w = HashMap::new();

    let mut ne: Vec<Arc> = Vec::new();
    let mut se: Vec<Arc> = Vec::new();
    let mut sw: Vec<Arc> = Vec::new();
    let mut nw: Vec<Arc> = Vec::new();
    let mut part_horiz = AtomicUsize::new(0);
    let mut part_vert = AtomicUsize::new(0);
    let mut part_diag = AtomicUsize::new(0);

    let x_str = format!("X = {}", x);
    let y_str = format!("Y = {}", y);
    log_info!(
        "Partitioning arcs along {}, {}\n",
        x_str.bold(),
        y_str.bold()
    );

    let mut candidates = 0;
    let mut north = 0;
    let mut east = 0;
    let mut south = 0;
    let mut west = 0;
    for &pip in pips {
        let loc = ctx.pip_location(pip);
        if (loc.x == x || loc.y == y) && x_bounds.contains(&loc.x) && y_bounds.contains(&loc.y) {
            let dir = ctx.pip_direction(pip);

            // This pip seems internal; skip it.
            if dir.x == 0 && dir.y == 0 {
                continue;
            }

            candidates += 1;

            if dir.x < 0 {
                north += 1;
                pips_n
                    .entry((loc.x, loc.y))
                    .and_modify(|pip_list: &mut Vec<(npnr::PipId, AtomicUsize)>| {
                        pip_list.push((pip, AtomicUsize::new(0)))
                    })
                    .or_insert_with(|| vec![(pip, AtomicUsize::new(0))]);
            }

            if dir.x > 0 {
                south += 1;
                pips_s
                    .entry((loc.x, loc.y))
                    .and_modify(|pip_list: &mut Vec<(npnr::PipId, AtomicUsize)>| {
                        pip_list.push((pip, AtomicUsize::new(0)))
                    })
                    .or_insert_with(|| vec![(pip, AtomicUsize::new(0))]);
            }

            if dir.y < 0 {
                east += 1;
                pips_e
                    .entry((loc.x, loc.y))
                    .and_modify(|pip_list: &mut Vec<(npnr::PipId, AtomicUsize)>| {
                        pip_list.push((pip, AtomicUsize::new(0)))
                    })
                    .or_insert_with(|| vec![(pip, AtomicUsize::new(0))]);
            }

            if dir.y > 0 {
                west += 1;
                pips_w
                    .entry((loc.x, loc.y))
                    .and_modify(|pip_list: &mut Vec<(npnr::PipId, AtomicUsize)>| {
                        pip_list.push((pip, AtomicUsize::new(0)))
                    })
                    .or_insert_with(|| vec![(pip, AtomicUsize::new(0))]);
            }
        }
    }

    log_info!(
        "  Out of {} candidate pips:\n",
        candidates.to_string().bold()
    );
    log_info!("    {} are north-bound\n", north.to_string().bold());
    log_info!("    {} are east-bound\n", east.to_string().bold());
    log_info!("    {} are south-bound\n", south.to_string().bold());
    log_info!("    {} are west-bound\n", west.to_string().bold());

    let progress = ProgressBar::new(arcs.len() as u64);
    progress.set_style(
        ProgressStyle::with_template("[{elapsed}] [{bar:40.cyan/blue}] {msg}")
            .unwrap()
            .progress_chars("━╸ "),
    );

    let find_best_pip = |pips: &Vec<(npnr::PipId, AtomicUsize)>,
                         source_wire: npnr::WireId,
                         sink_wire: npnr::WireId| {
        let (selected_pip, pip_uses) = pips
            .iter()
            .min_by_key(|(pip, uses)| {
                let src_to_pip = ctx.estimate_delay(source_wire, ctx.pip_src_wire(*pip));
                let pip_to_snk = ctx.estimate_delay(ctx.pip_dst_wire(*pip), sink_wire);
                let uses = uses.load(std::sync::atomic::Ordering::Acquire);
                (1000.0 * (src_to_pip + ((uses + 1) as f32) * pip_to_snk)) as u64
            })
            .unwrap();
        pip_uses.fetch_add(1, std::sync::atomic::Ordering::Release);
        *selected_pip
    };

    let mut explored_pips = AtomicUsize::new(0);

    let partition_coords = Coord::new(x, y);

    let arcs = arcs
        .into_par_iter()
        .progress_with(progress)
        .flat_map(|arc| {
            let source_loc = arc.get_source_loc();
            let source_coords: Coord = source_loc.into();
            let source_is_north = source_coords.is_north_of(&partition_coords);
            let source_is_east = source_coords.is_east_of(&partition_coords);
            let sink_loc = arc.get_sink_loc();
            let sink_coords: Coord = sink_loc.into();
            let sink_is_north = sink_coords.is_north_of(&partition_coords);
            let sink_is_east = sink_coords.is_east_of(&partition_coords);
            if source_is_north == sink_is_north && source_is_east == sink_is_east {
                let seg = source_coords.segment_from(&Coord::new(x, y));
                vec![(seg, arc.clone())]
            } else if source_is_north != sink_is_north && source_is_east == sink_is_east {
                let middle = (x, (source_coords.y + sink_coords.y) / 2);
                let middle = (
                    middle.0.clamp(1, ctx.grid_dim_x() - 1),
                    middle.1.clamp(1, ctx.grid_dim_y() - 1),
                );
                let pips = match source_is_north {
                    true => pips_s.get(&middle).unwrap(),
                    false => pips_n.get(&middle).unwrap(),
                };

                let selected_pip = find_best_pip(pips, arc.get_source_wire(), arc.get_sink_wire());
                explored_pips.fetch_add(pips.len(), std::sync::atomic::Ordering::Relaxed);

                let (src_to_pip, pip_to_dst) = arc.split(ctx, selected_pip);
                let (seg1, seg2) = match (source_is_north, source_is_east) {
                    (true, true) => (Segment::Northeast, Segment::Southeast),
                    (true, false) => (Segment::Northwest, Segment::Southwest),
                    (false, true) => (Segment::Southeast, Segment::Northeast),
                    (false, false) => (Segment::Southwest, Segment::Northwest),
                };
                part_horiz.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                vec![(seg1, src_to_pip), (seg2, pip_to_dst)]
            } else if source_is_north == sink_is_north && source_is_east != sink_is_east {
                let middle = ((source_coords.x + sink_coords.x) / 2, y);
                let middle = (
                    middle.0.clamp(1, ctx.grid_dim_x() - 1),
                    middle.1.clamp(1, ctx.grid_dim_y() - 1),
                );
                let pips = match source_is_east {
                    true => pips_w.get(&middle).unwrap(),
                    false => pips_e.get(&middle).unwrap(),
                };

                let selected_pip = find_best_pip(pips, arc.get_source_wire(), arc.get_sink_wire());
                explored_pips.fetch_add(pips.len(), std::sync::atomic::Ordering::Relaxed);

                let (src_to_pip, pip_to_dst) = arc.split(ctx, selected_pip);
                let (seg1, seg2) = match (source_is_north, source_is_east) {
                    (true, true) => (Segment::Northeast, Segment::Northwest),
                    (true, false) => (Segment::Northwest, Segment::Northeast),
                    (false, true) => (Segment::Southeast, Segment::Southwest),
                    (false, false) => (Segment::Southwest, Segment::Southeast),
                };
                part_vert.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                vec![(seg1, src_to_pip), (seg2, pip_to_dst)]
            } else {
                let middle = (x, split_line_over_x((source_loc, sink_loc), x));
                let middle = (
                    middle.0.clamp(1, ctx.grid_dim_x() - 1),
                    middle.1.clamp(1, ctx.grid_dim_y() - 1),
                );
                let pips = match source_is_east {
                    true => pips_w.get(&middle).unwrap(),
                    false => pips_e.get(&middle).unwrap(),
                };

                let horiz_pip = find_best_pip(pips, arc.get_source_wire(), arc.get_sink_wire());
                explored_pips.fetch_add(pips.len(), std::sync::atomic::Ordering::Relaxed);

                let middle = (split_line_over_y((source_loc, sink_loc), y), y);
                let middle = (
                    middle.0.clamp(1, ctx.grid_dim_x() - 1),
                    middle.1.clamp(1, ctx.grid_dim_y() - 1),
                );
                let pips = match source_is_north {
                    true => pips_s.get(&middle).unwrap(),
                    false => pips_n.get(&middle).unwrap(),
                };

                let vert_pip = find_best_pip(pips, arc.get_source_wire(), arc.get_sink_wire());
                explored_pips.fetch_add(pips.len(), std::sync::atomic::Ordering::Relaxed);

                let horiz_loc: Coord = ctx.pip_location(horiz_pip).into();
                let horiz_is_east = horiz_loc.is_east_of(&partition_coords);
                let (src_to_mid1, mid1_to_mid2, mid2_to_dst) = if horiz_is_east == source_is_east {
                    let (a, b) = arc.split(ctx, horiz_pip);
                    let (b, c) = b.split(ctx, vert_pip);
                    (a, b, c)
                } else {
                    let (a, b) = arc.split(ctx, vert_pip);
                    let (b, c) = b.split(ctx, horiz_pip);
                    (a, b, c)
                };
                let (seg1, seg2, seg3) = match (source_is_north, source_is_east, horiz_is_east) {
                    (true, true, true) => {
                        (Segment::Northeast, Segment::Southeast, Segment::Southwest)
                    }
                    (true, true, false) => {
                        (Segment::Northeast, Segment::Northwest, Segment::Southwest)
                    }
                    (true, false, true) => {
                        (Segment::Northwest, Segment::Northeast, Segment::Southeast)
                    }
                    (true, false, false) => {
                        (Segment::Northwest, Segment::Southwest, Segment::Southeast)
                    }
                    (false, true, true) => {
                        (Segment::Southeast, Segment::Northeast, Segment::Northwest)
                    }
                    (false, true, false) => {
                        (Segment::Southeast, Segment::Southwest, Segment::Northwest)
                    }
                    (false, false, true) => {
                        (Segment::Southwest, Segment::Southeast, Segment::Northeast)
                    }
                    (false, false, false) => {
                        (Segment::Southwest, Segment::Northwest, Segment::Northeast)
                    }
                };
                part_diag.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                vec![
                    (seg1, src_to_mid1),
                    (seg2, mid1_to_mid2),
                    (seg3, mid2_to_dst),
                ]
            }
        })
        .collect::<Vec<_>>();

    for (segment, arc) in arcs {
        match segment {
            Segment::Northeast => ne.push(arc),
            Segment::Southeast => se.push(arc),
            Segment::Southwest => sw.push(arc),
            Segment::Northwest => nw.push(arc),
        }
    }

    log_info!(
        "  {} pips explored\n",
        explored_pips.get_mut().to_string().bold()
    );

    let north = ne.len() + nw.len();
    let south = se.len() + sw.len();

    let nets = (north + south) as f64;

    let ne_dist = ((ne.len() as f64) / nets) - 0.25;
    let se_dist = ((se.len() as f64) / nets) - 0.25;
    let sw_dist = ((sw.len() as f64) / nets) - 0.25;
    let nw_dist = ((nw.len() as f64) / nets) - 0.25;

    let ne_str = ne.len().to_string();
    let se_str = se.len().to_string();
    let sw_str = sw.len().to_string();
    let nw_str = nw.len().to_string();

    let dist_str = |dist: f64| {
        if dist > 0.20 {
            "(way too many nets)".red()
        } else if dist > 0.05 {
            "(too many nets)".yellow()
        } else if dist < -0.05 {
            "(too few nets)".yellow()
        } else if dist < -0.20 {
            "(way too few nets)".red()
        } else {
            "(balanced)".green()
        }
    };

    log_info!(
        "  {} arcs partitioned horizontally\n",
        part_horiz.get_mut().to_string().bold()
    );
    log_info!(
        "  {} arcs partitioned vertically\n",
        part_vert.get_mut().to_string().bold()
    );
    log_info!(
        "  {} arcs partitioned both ways\n",
        part_diag.get_mut().to_string().bold()
    );
    log_info!(
        "  {} arcs in the northeast {}\n",
        ne_str.color(if ne_dist.abs() > 0.20 {
            colored::Color::Red
        } else if ne_dist.abs() > 0.05 {
            colored::Color::Yellow
        } else {
            colored::Color::Green
        }),
        dist_str(ne_dist)
    );
    log_info!(
        "  {} arcs in the southeast {}\n",
        se_str.color(if se_dist.abs() > 0.20 {
            colored::Color::Red
        } else if se_dist.abs() > 0.05 {
            colored::Color::Yellow
        } else {
            colored::Color::Green
        }),
        dist_str(se_dist)
    );
    log_info!(
        "  {} arcs in the southwest {}\n",
        sw_str.color(if sw_dist.abs() > 0.20 {
            colored::Color::Red
        } else if sw_dist.abs() > 0.05 {
            colored::Color::Yellow
        } else {
            colored::Color::Green
        }),
        dist_str(sw_dist)
    );
    log_info!(
        "  {} arcs in the northwest {}\n",
        nw_str.color(if nw_dist.abs() > 0.20 {
            colored::Color::Red
        } else if nw_dist.abs() > 0.05 {
            colored::Color::Yellow
        } else {
            colored::Color::Green
        }),
        dist_str(nw_dist)
    );

    (ne, se, sw, nw)
}