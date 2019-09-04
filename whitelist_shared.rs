#![allow(improper_ctypes)]

use crate::handle::handle;
use std::collections::HashMap;
use std::ffi::CStr;
use std::os::raw::c_char;
use std::sync::RwLock;

fn whitelist() -> &'static RwLock<HashMap<&'static CStr, usize>> {
	use std::sync::ONCE_INIT;
	use std::sync::Once;

	extern {
		fn whitelist_shared_init(_: *mut HashMap<&CStr, usize>);
	}

	static INIT: Once = ONCE_INIT;
	static mut WHITEMAP: Option<RwLock<HashMap<&CStr, usize>>> = None;
	INIT.call_once(|| {
		let whitemap = unsafe {
			WHITEMAP.get_or_insert(RwLock::default())
		};
		let mut whitemap = whitemap.write().unwrap();
		unsafe {
			whitelist_shared_init(&mut *whitemap);
		}
		drop(whitemap);
	});
	unsafe {
		WHITEMAP.as_ref()
	}.unwrap()
}

#[no_mangle]
extern fn whitelist_shared_get(symbol: *const c_char) -> usize {
	let whitelist = whitelist().read().unwrap();
	if symbol.is_null() {
		usize::max_value()
	} else {
		*whitelist.get(unsafe {
			CStr::from_ptr(symbol)
		}).unwrap_or(&usize::max_value())
	}
}

#[no_mangle]
extern fn whitelist_shared_insert(
	whitelist: Option<&mut HashMap<&CStr, usize>>,
	symbol: *const c_char,
	replacement: usize,
) {
	whitelist.unwrap().entry(unsafe {
		CStr::from_ptr(symbol)
	}).or_insert(replacement);
}

#[no_mangle]
extern fn whitelist_so_insert(handle: *const handle) {
	extern {
		fn whitelist_so_insert_with(
			_: *const handle,
			_: *mut HashMap<&CStr, usize>,
			_: unsafe extern fn(*const c_char) -> bool,
			_: bool,
		);
	}

	extern fn yes(_: *const c_char) -> bool {
		true
	}

	let mut whitelist = whitelist().write().unwrap();
	unsafe {
		whitelist_so_insert_with(handle, &mut *whitelist, yes, false);
	}
	drop(whitelist);
}