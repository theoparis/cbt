pub struct MyStruct {
	pub x: i32,
	pub y: i32,
}

impl Default for MyStruct {
	fn default() -> Self {
		MyStruct { x: 0, y: 0 }
	}
}

pub fn add(a: i32, b: i32) -> i32 {
	a + b
}

pub fn greet(name: String) -> String {
	format!("Hello, {}", name)
}
