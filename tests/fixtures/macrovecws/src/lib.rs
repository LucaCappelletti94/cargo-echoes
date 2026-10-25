mod custom {
    macro_rules! vec {
        (1) => {
            10
        };
        (1,) => {
            20
        };
    }
    pub(crate) use vec;
}

/// ```
/// mod custom {
///     macro_rules! vec {
///         (1) => { 10 };
///         (1,) => { 20 };
///     }
///     pub(crate) use vec;
/// }
/// fn main() {
///     let one = custom::vec!(1);
///     println!("{one}");
/// }
/// ```
struct First;

/// ```
/// mod custom {
///     macro_rules! vec {
///         (1) => { 10 };
///         (1,) => { 20 };
///     }
///     pub(crate) use vec;
/// }
/// fn main() {
///     let one = custom::vec!(1,);
///     println!("{one}");
/// }
/// ```
struct Second;

/// ```
/// fn main() {
///     let values = vec![1, 2, 3];
///     println!("{values:?}");
/// }
/// ```
struct Standard;

/// ```
/// fn main() {
///     let values = vec![1, 2, 3,];
///     println!("{values:?}");
/// }
/// ```
struct StandardComma;

fn first() -> u32 {
    custom::vec!(1)
}

fn second() -> u32 {
    custom::vec!(1,)
}

fn equal_first() -> u32 {
    1 + 1
}

fn equal_second() -> u32 {
    1 + 1
}
