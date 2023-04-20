class BBAWriter:
	def __init__(self, f):
		self.f = f
	def pre(self, s):
		print(f"pre {s}", file=self.f)
	def post(self, s):
		print(f"post {s}", file=self.f)
	def push(self, s):
		print(f"push {s}", file=self.f)
	def offset32(self):
		print("offset32", file=self.f)
	def ref(self, r, comment=""):
		print(f"ref {r} {comment}", file=self.f)
	def slice(self, r, size, comment=""):
		print(f"ref {r} {comment}", file=self.f)
		print(f"u32 {size}")
	def str(self, s, comment=""):
		print(f"str |{s}| {comment}", file=self.f)
	def align(self):
		print("align", file=self.f)
	def label(self, s):
		print(f"label {s}", file=self.f)
	def u8(self, n, comment=""):
		print(f"u8 {n} {comment}", file=self.f)
	def u16(self, n, comment=""):
		print(f"u16 {n} {comment}", file=self.f)
	def u32(self, n, comment=""):
		print(f"u32 {n} {comment}", file=self.f)
	def pop(self):
		print("pop", file=self.f)
