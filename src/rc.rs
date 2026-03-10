use std::ops::Deref;

pub struct Inner<T> {
    count: usize,
    value: T,
}

pub struct Rc<T> {
    inner: *mut Inner<T>,
}

impl<T> Rc<T> {
    pub fn new(value: T) -> Self {
        Rc {
            inner: Box::into_raw(Box::new(Inner { count: 1, value })),
        }
    }
}

impl<T> Clone for Rc<T> {
    fn clone(&self) -> Self {
        unsafe { (*self.inner).count += 1 }
        Rc { inner: self.inner }
    }
}

impl<T> Drop for Rc<T> {
    fn drop(&mut self) {
        unsafe {
            (*self.inner).count -= 1;
            if (*self.inner).count == 0 {
                let _ = Box::from_raw(self.inner);
            }
        }
    }
}

impl<T> Deref for Rc<T> {
    type Target = T;
    fn deref(&self) -> &Self::Target {
        unsafe { &(*self.inner).value }
    }
}

#[test]
fn test() {
    let a = Rc::new(String::from("hello"));
    let b = a.clone();

    println!("a: {}, length: {}", *a, a.len());
    println!("b: {}", *b);
}
