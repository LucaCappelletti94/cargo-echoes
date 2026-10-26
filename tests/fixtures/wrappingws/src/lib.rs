/// ```
/// fn f(x: u32, y: u32) -> u32 { x.wrapping_add(y) }
/// ```
pub struct Written;

/// ```
/// fn f(x: u32, y: u32) -> u32 { y.wrapping_add(x) }
/// ```
pub struct Reordered;

/// ```
/// fn f(x: u32) -> u32 { x.wrapping_add(7u32) }
/// ```
pub struct LiteralWritten;

/// ```
/// fn f(x: u32) -> u32 { 7u32.wrapping_add(x) }
/// ```
pub struct LiteralReordered;

/// ```
/// fn f(x: i8, y: i8) -> i8 { x.wrapping_add(y) }
/// ```
pub struct SignedWritten;

/// ```
/// fn f(x: i8, y: i8) -> i8 { y.wrapping_add(x) }
/// ```
pub struct SignedReordered;

/// ```
/// fn f(x: u32, y: u32) -> u32 { x.wrapping_sub(y) }
/// ```
pub struct SubtractionWritten;

/// ```
/// fn f(x: u32, y: u32) -> u32 { y.wrapping_sub(x) }
/// ```
pub struct SubtractionReordered;

/// ```
/// fn f(x: u32, y: u32, z: u32) -> u32 { x.wrapping_add(y).wrapping_add(z) }
/// ```
pub struct LeftNested;

/// ```
/// fn f(x: u32, y: u32, z: u32) -> u32 { x.wrapping_add(y.wrapping_add(z)) }
/// ```
pub struct RightNested;

/// ```
/// #[derive(Clone, Copy)]
/// struct T(u32);
/// impl T {
///     fn wrapping_add(self, other: T) -> T { T(self.0.wrapping_sub(other.0)) }
/// }
/// fn f(x: T, y: T) -> T { x.wrapping_add(y) }
/// ```
pub struct CustomWritten;

/// ```
/// #[derive(Clone, Copy)]
/// struct T(u32);
/// impl T {
///     fn wrapping_add(self, other: T) -> T { T(self.0.wrapping_sub(other.0)) }
/// }
/// fn f(x: T, y: T) -> T { y.wrapping_add(x) }
/// ```
pub struct CustomReordered;

/// ```
/// fn a() -> u32 { println!("a"); 1 }
/// fn b() -> u32 { println!("b"); 2 }
/// fn f() -> u32 { a().wrapping_add(b()) }
/// ```
pub struct EffectWritten;

/// ```
/// fn a() -> u32 { println!("a"); 1 }
/// fn b() -> u32 { println!("b"); 2 }
/// fn f() -> u32 { b().wrapping_add(a()) }
/// ```
pub struct EffectReordered;

pub fn written(x: u32, y: u32) -> u32 { x.wrapping_add(y) }
pub fn reordered(x: u32, y: u32) -> u32 { y.wrapping_add(x) }
pub fn literal_written(x: u32) -> u32 { x.wrapping_add(7u32) }
pub fn literal_reordered(x: u32) -> u32 { 7u32.wrapping_add(x) }

pub mod signed {
    pub fn written(x: i8, y: i8) -> i8 { x.wrapping_add(y) }
    pub fn reordered(x: i8, y: i8) -> i8 { y.wrapping_add(x) }
}

pub mod subtraction {
    pub fn written(x: u32, y: u32) -> u32 { x.wrapping_sub(y) }
    pub fn reordered(x: u32, y: u32) -> u32 { y.wrapping_sub(x) }
}

pub mod custom {
    #[derive(Clone, Copy)]
    pub struct T(u32);
    impl T {
        pub fn wrapping_add(self, other: T) -> T { T(self.0.wrapping_sub(other.0)) }
    }
    pub fn written(x: T, y: T) -> T { x.wrapping_add(y) }
    pub fn reordered(x: T, y: T) -> T { y.wrapping_add(x) }
}

pub mod effects {
    fn a() -> u32 { println!("a"); 1 }
    fn b() -> u32 { println!("b"); 2 }
    pub fn written() -> u32 { a().wrapping_add(b()) }
    pub fn reordered() -> u32 { b().wrapping_add(a()) }
}
