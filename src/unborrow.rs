//! Durka's `unborrow!` macro.

/// Explicitly precompute a method's arguments before the call so that borrowck sees them the same
/// way that trans does.
///
/// Examples
/// =======
///
/// ```
/// # #[macro_use] extern crate unborrow;
/// # fn main() {
/// let mut v = vec![1, 2, 3];
///
/// // this line would cause an error because borrowck consider `v` borrowed by `reserve`
/// // during its parameter list
/// // v.reserve(v.capacity()); //~ERROR cannot borrow `v`
/// // but wrap the call in unborrow!() and it works!
/// unborrow!(v.reserve(v.capacity()));
/// assert!(v.capacity() >= 6);
/// assert_eq!(v, [1, 2, 3]);
///
/// // similar to the above, both v.len()-1 and v[0]+41 require borrowing `v` and we can't
/// // do that while borrowck thinks is is mutably borrowed by `insert`
/// // v.insert(v.len() - 1, v[0] + 41); //~ERROR cannot borrow `v`
/// // but wrap the call in unborrow!() and it works!
/// unborrow!(v.insert(v.len() - 1, v[0] + 41));
/// assert_eq!(v, [1, 2, 42, 3]);
///
/// // it also works for nested objects!
/// struct Wrapper { v: Vec<i32> }
/// let mut w = Wrapper { v: vec![1, 2, 3] };
/// unborrow!(w.v.reserve(w.v.capacity()));
///
/// // ...and with free functions! (the first argument is assumed to be the mutable borrow)
/// use std::mem;
/// unborrow!(mem::replace(&mut v, v.clone()));
///
/// # }
/// ```
macro_rules! unborrow {
    // =========================================================================================================
    // PRIVATE RULES

    // This rule fires when we have parsed all the arguments.
    // It just falls through to output stage.
    // (FIXME could fold the output rule into this one to reduce recursion)
    (@parse () -> ($names:tt $lets:tt) $($thru:tt)*) => {
        unborrow!(@out $names $lets $($thru)*)
    };

    // Parse an argument and continue parsing
    // This is the key rule, assigning a name for the argument and generating the let statement.
    (@parse ($arg:expr, $($rest:tt)*) -> ([$($names:ident),*] [$($lets:stmt);*]) $($thru:tt)*) => {
        unborrow!(@parse ($($rest)*) -> ([$($names,)* arg] [$($lets;)* let arg = $arg]) $($thru)*)
        //                                            ^                    ^
        // Right here an ident is created out of thin air using hygiene.
        // Every time the macro recurses, we get a new syntax context, so "arg" is actually a new identifier!
    };

    // Output stage for free functions.
    // Assembles the let statements and variable names into a block which computes the arguments,
    // calls the method, and returns its result.
    (@out [$($names:ident),*] [$($lets:stmt);*] ($($meth:ident)::+) $arg1:expr) => {{
        $($lets;)*
        $($meth)::+($arg1, $($names),*)
    }};

    // Output stage for object methods.
    (@out [$($names:ident),*] [$($lets:stmt);*] $($obj:ident).+) => {{
        $($lets;)*
        $($obj).+($($names),*)
    }};

    // =========================================================================================================
    // PUBLIC RULES

    // Macro entry point for object methods.
    ($($obj:ident).+ ($($args:expr),*)) => {
        unborrow!(@parse ($($args,)*) -> ([] []) $($obj).+)
        //                |               |  |   ^ info about the method call, saved for later
        //                |               |  ^ generated let statements
        //                |               ^ generated argument names
        //                ^ arguments to be parsed
    };

    // Macro entry point for free functions.
    ($($meth:ident)::+ ($arg1:expr, $($args:expr),*)) => {
        unborrow!(@parse ($($args,)*) -> ([] []) ($($meth)::+) $arg1)
    };
}
