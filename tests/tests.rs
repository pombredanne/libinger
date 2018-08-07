#![cfg_attr(not(test), allow(dead_code))]

extern crate ucontext;

fn main() {
	getcontext_donothing();
	getcontext_setcontext();
	getcontext_succeedatnothing();
}

#[cfg_attr(test, test)]
fn getcontext_donothing() {
	use ucontext::getcontext;

	let mut reached = false;
	getcontext(|_| reached = true, || unreachable!()).unwrap();
	assert!(reached);
}

#[cfg_attr(test, test)]
fn getcontext_setcontext() {
	use ucontext::getcontext;
	use ucontext::setcontext;

	let mut reached = false;
	getcontext(
		|context| {
			setcontext(context);
			unreachable!();
		},
		|| reached = true,
	).unwrap();
	assert!(reached);
}

#[cfg_attr(test, test)]
fn getcontext_succeedatnothing() {
	use ucontext::getcontext;
	use ucontext::setcontext;

	let mut invalid = None;
	getcontext(|context| invalid = Some(context), || unreachable!()).unwrap();
	assert!(setcontext(invalid.unwrap()).is_none());
}
