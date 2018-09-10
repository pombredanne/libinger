#![feature(test)]

extern crate libc;
extern crate test;
extern crate timetravel;

#[allow(dead_code)]
mod lifetimes;

use libc::MINSIGSTKSZ;
use libc::SIGSTKSZ;
use libc::siginfo_t;
use libc::ucontext_t;
use std::mem::uninitialized;
use std::os::raw::c_int;
use std::ptr::read_volatile;
use std::ptr::write_volatile;
use test::Bencher;
use timetravel::HandlerContext;

#[bench]
fn get_native(lo: &mut Bencher) {
	use libc::getcontext;

	lo.iter(|| unsafe {
		getcontext(&mut uninitialized());
	});
}

#[bench]
fn get_timetravel(lo: &mut Bencher) {
	use timetravel::getcontext;

	lo.iter(|| getcontext(|_| (), || ()));
}

fn get_helper<T, F: FnMut(ucontext_t) -> T>(mut fun: F) {
	use libc::getcontext;

	let mut initial = true;
	unsafe {
		let mut context = uninitialized();
		getcontext(&mut context);
		if read_volatile(&initial) {
			write_volatile(&mut initial, false);
			fun(context);
		}
	}
}

#[bench]
fn getset_native(lo: &mut Bencher) {
	use libc::setcontext;

	lo.iter(|| get_helper(|context| unsafe {
		setcontext(&context)
	}));
}

#[bench]
fn getset_timetravel(lo: &mut Bencher) {
	use timetravel::getcontext;
	use timetravel::setcontext;

	lo.iter(|| getcontext(|context| setcontext(&context), || None));
}

fn make_helper<T, F: FnMut(ucontext_t) -> T>(stack: &mut [u8], gated: extern "C" fn(), mut fun: F) {
	use libc::getcontext;
	use libc::makecontext;

	get_helper(|mut context| {
		let mut gate = unsafe {
			uninitialized()
		};
		unsafe {
			getcontext(&mut gate);
		}
		gate.uc_stack.ss_sp = stack.as_mut_ptr() as _;
		gate.uc_stack.ss_size = stack.len();
		gate.uc_link = &mut context;
		unsafe {
			makecontext(&mut gate, gated, 0);
		}
		fun(gate);
	});
}

#[bench]
fn make_native(lo: &mut Bencher) {
	extern "C" fn stub() {}

	lo.iter(|| make_helper(&mut [0u8; MINSIGSTKSZ][..], stub, |_| ()));
}

#[bench]
fn make_timetravel(lo: &mut Bencher) {
	use timetravel::makecontext;

	let mut stack = [0u8; MINSIGSTKSZ];
	lo.iter(|| makecontext(&mut stack[..], |_| (), || ()));
}

#[bench]
fn makeset_native(lo: &mut Bencher) {
	use libc::setcontext;

	extern "C" fn stub() {}

	lo.iter(|| make_helper(&mut [0u8; MINSIGSTKSZ][..], stub, |gate| unsafe {
		setcontext(&gate)
	}));
}

#[bench]
fn makeset_timetravel(lo: &mut Bencher) {
	use timetravel::makecontext;
	use timetravel::setcontext;

	let mut stack = [0u8; MINSIGSTKSZ];
	lo.iter(|| makecontext(&mut stack[..], |gate| panic!(setcontext(&gate)), || ()));
}

#[bench]
fn swapsig_fork(lo: &mut Bencher) {
	use libc::fork;
	use libc::waitpid;
	use std::process::exit;
	use std::ptr::null_mut;

	lo.iter(|| {
		let pid = unsafe {
			fork()
		};
		if pid == 0 {
			exit(0);
		} else {
			unsafe {
				waitpid(pid, null_mut(), 0);
			}
		}
	});
}

fn swapsig_helper(handler: extern "C" fn(c_int, Option<&mut siginfo_t>, Option<&mut HandlerContext>)) {
	use libc::SA_SIGINFO;
	use libc::SIGUSR1;
	use libc::pthread_kill;
	use libc::pthread_self;
	use libc::sigaction;
	use std::mem::zeroed;
	use std::ptr::null_mut;

	let config = sigaction {
		sa_flags: SA_SIGINFO,
		sa_sigaction: handler as _,
		sa_restorer: None,
		sa_mask: unsafe {
			zeroed()
		},
	};
	unsafe {
		sigaction(SIGUSR1, &config, null_mut());
		pthread_kill(pthread_self(), SIGUSR1);
	}
}

#[bench]
fn swapsig_native(lo: &mut Bencher) {
	use libc::getcontext;
	use libc::setcontext;
	use lifetimes::unbound_mut;
	use std::mem::transmute;
	use timetravel::Swap;

	static mut CHECKPOINT: Option<&'static mut ucontext_t> = None;
	static mut GATE: Option<&'static mut ucontext_t> = None;
	static mut LO: Option<&'static mut Bencher> = None;

	extern "C" fn checkpoint(_: c_int, _: Option<&mut siginfo_t>, context: Option<&mut ucontext_t>) {
		let context = context.unwrap();
		unsafe {
			GATE.as_mut()
		}.unwrap().swap(context);

		let mut checkpoint: ucontext_t = **unsafe {
			CHECKPOINT.as_ref()
		}.unwrap();
		checkpoint.swap(context);
	}

	extern "C" fn restore(_: c_int, _: Option<&mut siginfo_t>, context: Option<&mut ucontext_t>) {
		unsafe {
			GATE.as_mut()
		}.unwrap().swap(context.unwrap());
	}

	extern "C" fn benchmark() {
		let checkpoint: extern "C" fn(c_int, Option<&mut siginfo_t>, Option<&mut ucontext_t>) = checkpoint;
		unsafe {
			LO.as_mut()
		}.unwrap().iter(|| swapsig_helper(unsafe {
			transmute(checkpoint)
		}));
	}

	let restore: extern "C" fn(c_int, Option<&mut siginfo_t>, Option<&mut ucontext_t>) = restore;
	unsafe {
		LO = Some(unbound_mut(lo));
	}
	make_helper(&mut [0u8; SIGSTKSZ][..], benchmark, |mut gate| {
		unsafe {
			GATE = Some(unbound_mut(&mut gate));
		}
		get_helper(|mut checkpoint| unsafe {
			CHECKPOINT = Some(unbound_mut(&mut checkpoint));
			setcontext(&mut gate);
		});

		let mut checkpoint = unsafe {
			uninitialized()
		};
		unsafe {
			CHECKPOINT = Some(unbound_mut(&mut checkpoint));
			getcontext(&mut checkpoint);
		}
		swapsig_helper(unsafe {
			transmute(restore)
		});
	});
}

#[bench]
fn swapsig_timetravel(lo: &mut Bencher) {
	use lifetimes::unbound_mut;
	use timetravel::Context;
	use timetravel::Swap;
	use timetravel::makecontext;
	use timetravel::restorecontext;
	use timetravel::setcontext;
	use timetravel::sigsetcontext;

	static mut CHECKPOINT: Option<Context<Box<[u8]>>> = None;
	static mut GOING: bool = true;
	static mut LO: Option<&'static mut Bencher> = None;

	extern "C" fn handler(_: c_int, _: Option<&mut siginfo_t>, context: Option<&mut HandlerContext>) {
		unsafe {
			CHECKPOINT.as_mut()
		}.unwrap().swap(context.unwrap());
	}

	let stack: Box<[_]> = Box::new([0u8; SIGSTKSZ]);
	drop(makecontext(
		stack,
		|gate| {
			let gate = unsafe {
				CHECKPOINT.get_or_insert(gate)
			};
			unsafe {
				LO = Some(unbound_mut(lo));
			}
			panic!(setcontext(gate));
		},
		|| {
			unsafe {
				LO.as_mut()
			}.unwrap().iter(|| swapsig_helper(handler));
			unsafe {
				GOING = false;
			}
		},
	));

	while {
		drop(restorecontext(
			unsafe {
				CHECKPOINT.take()
			}.unwrap(),
			|checkpoint| {
				let checkpoint = unsafe {
					CHECKPOINT.get_or_insert(checkpoint)
				};
				panic!(sigsetcontext(checkpoint));
			},
		));

		unsafe {
			GOING
		}
	} {}
}