extern crate alloc;

use alloc::boxed::Box;
use test_crate::MyStruct;

#[no_mangle]
pub extern "C" fn my_struct_new() -> *mut MyStruct {
	Box::into_raw(Box::new(MyStruct::default()))
}

#[no_mangle]
pub extern "C" fn my_struct_free(obj: *mut MyStruct) {
	if !obj.is_null() {
		unsafe {
			let _ = Box::from_raw(obj);
		}
	}
}

#[no_mangle]
pub extern "C" fn add(a: i32, b: i32) -> i32 {
	use test_crate::add;
	let a = unsafe { core::mem::transmute::<i32, _>(a) };
	let b = unsafe { core::mem::transmute::<i32, _>(b) };
	let result = add(a, b);
	unsafe { std::mem::transmute::<_, i32>(result) }
}

#[no_mangle]
pub extern "C" fn greet(
	name: *mut core::ffi::c_char,
) -> *mut core::ffi::c_char {
	use test_crate::greet;
	let name = unsafe {
		core::ffi::CStr::from_ptr(name)
			.to_string_lossy()
			.into_owned()
	};
	let result = greet(name);
	let result = std::ffi::CString::new(result).unwrap().into_raw();
	result
}
