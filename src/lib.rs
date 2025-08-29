#![no_std]

pub type Mutex<T> = lock_api::Mutex<RawMutex, T>;

pub const fn const_mutex<T>(val: T) -> Mutex<T> {
    Mutex::const_new(<RawMutex as lock_api::RawMutex>::INIT, val)
}

pub type MutexGuard<'a, T> = lock_api::MutexGuard<'a, RawMutex, T>;

pub type MappedMutexGuard<'a, T> = lock_api::MappedMutexGuard<'a, RawMutex, T>;

/// Raw mutex type backed with ariel-os lock.
pub struct RawMutex(ariel_os_threads::sync::Lock);

unsafe impl lock_api::RawMutex for RawMutex {
    const INIT: Self = RawMutex(ariel_os_threads::sync::Lock::new());

    type GuardMarker = lock_api::GuardNoSend;

    #[inline]
    fn lock(&self) {
        self.0.acquire();
    }

    #[inline]
    fn try_lock(&self) -> bool {
        self.0.try_acquire()
    }

    #[inline]
    unsafe fn unlock(&self) {
        self.0.release();
    }

    #[inline]
    fn is_locked(&self) -> bool {
        self.0.is_locked()
    }
}

/// Tests are copied from parking_lot.
#[cfg(test)]
mod tests {
    extern crate alloc;
    extern crate std;

    use crate::Mutex;
    use alloc::sync::Arc;
    use alloc::{format, vec};
    use core::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::mpsc::channel;
    use std::thread;

    #[derive(Eq, PartialEq, Debug)]
    struct NonCopy(i32);

    #[test]
    fn smoke() {
        let m = Mutex::new(());
        drop(m.lock());
        drop(m.lock());
    }

    #[test]
    fn try_lock() {
        let m = Mutex::new(());
        *m.try_lock().unwrap() = ();
    }

    #[test]
    fn test_into_inner() {
        let m = Mutex::new(NonCopy(10));
        assert_eq!(m.into_inner(), NonCopy(10));
    }

    #[test]
    fn test_into_inner_drop() {
        struct Foo(Arc<AtomicUsize>);
        impl Drop for Foo {
            fn drop(&mut self) {
                self.0.fetch_add(1, Ordering::SeqCst);
            }
        }
        let num_drops = Arc::new(AtomicUsize::new(0));
        let m = Mutex::new(Foo(num_drops.clone()));
        assert_eq!(num_drops.load(Ordering::SeqCst), 0);
        {
            let _inner = m.into_inner();
            assert_eq!(num_drops.load(Ordering::SeqCst), 0);
        }
        assert_eq!(num_drops.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn test_get_mut() {
        let mut m = Mutex::new(NonCopy(10));
        *m.get_mut() = NonCopy(20);
        assert_eq!(m.into_inner(), NonCopy(20));
    }

    #[test]
    fn test_mutex_arc_nested() {
        // Tests nested mutexes and access
        // to underlying data.
        let arc = Arc::new(Mutex::new(1));
        let arc2 = Arc::new(Mutex::new(arc));
        let (tx, rx) = channel();
        let _t = thread::spawn(move || {
            let lock = arc2.lock();
            let lock2 = lock.lock();
            assert_eq!(*lock2, 1);
            tx.send(()).unwrap();
        });
        rx.recv().unwrap();
    }

    #[test]
    fn test_mutex_arc_access_in_unwind() {
        let arc = Arc::new(Mutex::new(1));
        let arc2 = arc.clone();
        let _ = thread::spawn(move || {
            struct Unwinder {
                i: Arc<Mutex<i32>>,
            }
            impl Drop for Unwinder {
                fn drop(&mut self) {
                    *self.i.lock() += 1;
                }
            }
            let _u = Unwinder { i: arc2 };
            panic!();
        })
        .join();
        let lock = arc.lock();
        assert_eq!(*lock, 2);
    }

    #[test]
    fn test_mutex_unsized() {
        let mutex: &Mutex<[i32]> = &Mutex::new([1, 2, 3]);
        {
            let b = &mut *mutex.lock();
            b[0] = 4;
            b[2] = 5;
        }
        let comp: &[i32] = &[4, 2, 5];
        assert_eq!(&*mutex.lock(), comp);
    }

    #[test]
    fn test_mutexguard_sync() {
        fn sync<T: Sync>(_: T) {}

        let mutex = Mutex::new(());
        sync(mutex.lock());
    }

    #[test]
    fn test_mutex_debug() {
        let mutex = Mutex::new(vec![0u8, 10]);

        assert_eq!(format!("{:?}", mutex), "Mutex { data: [0, 10] }");
        let _lock = mutex.lock();
        assert_eq!(format!("{:?}", mutex), "Mutex { data: <locked> }");
    }
}
