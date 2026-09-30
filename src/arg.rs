/// A value that can be sent to the server as one command argument.
///
/// Every argument travels as a byte string, so numbers are sent in their
/// decimal form: `client.set("A", 1)` sends `SET A 1`.
pub trait ToArg {
    fn to_arg(&self) -> Vec<u8>;
}

impl ToArg for str {
    fn to_arg(&self) -> Vec<u8> {
        self.as_bytes().to_vec()
    }
}

impl ToArg for String {
    fn to_arg(&self) -> Vec<u8> {
        self.as_bytes().to_vec()
    }
}

impl ToArg for [u8] {
    fn to_arg(&self) -> Vec<u8> {
        self.to_vec()
    }
}

impl<const N: usize> ToArg for [u8; N] {
    fn to_arg(&self) -> Vec<u8> {
        self.to_vec()
    }
}

impl ToArg for Vec<u8> {
    fn to_arg(&self) -> Vec<u8> {
        self.clone()
    }
}

impl<T: ToArg + ?Sized> ToArg for &T {
    fn to_arg(&self) -> Vec<u8> {
        (**self).to_arg()
    }
}

macro_rules! to_arg_via_display {
    ($($t:ty),*) => {$(
        impl ToArg for $t {
            fn to_arg(&self) -> Vec<u8> {
                self.to_string().into_bytes()
            }
        }
    )*};
}

to_arg_via_display!(i8, i16, i32, i64, i128, isize, u8, u16, u32, u64, u128, usize, f32, f64);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn converts_arguments() {
        assert_eq!("key".to_arg(), b"key");
        assert_eq!(String::from("čau").to_arg(), "čau".as_bytes());
        assert_eq!(b"\x00\xff".to_arg(), b"\x00\xff");
        assert_eq!((-42).to_arg(), b"-42");
        assert_eq!(1.5.to_arg(), b"1.5");
    }
}
