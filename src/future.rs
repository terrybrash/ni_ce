use std::sync::{Arc, Condvar, Mutex};

enum FutureStatus<T> {
    Returned(T),
    Dropped,
}

/// A handle to a value to be returned at a later time.
///
/// `Future` is the receiver of a value sent by [`FutureLock`].
///
/// [`FutureLock`]: struct.FutureLock.html
pub struct Future<T> {
    result: Arc<(Mutex<Option<FutureStatus<T>>>, Condvar)>,
}

impl<T> Future<T> {
    /// Create a `Future` and its corresponding [`FutureLock`].
    ///
    /// The [`FutureLock`] is meant to be sent to a separate thread where a return value will be
    /// created and sent back using the lock.
    ///
    /// [`FutureLock`]: struct.FutureLock.html
    pub fn await() -> (Future<T>, FutureLock<T>) {
        let future = Future {
            result: Arc::new((Mutex::new(None), Condvar::new())),
        };

        let lock = FutureLock::new(future.result.clone());

        (future, lock)
    }

    /// Wait for the paired [`FutureLock`] to either return a value with [`FutureLock::send`] or
    /// drop.
    ///
    /// [`FutureLock`]: struct.FutureLock.html
    /// [`FutureLock::send`]: struct.FutureLock.html#method.send
    pub fn wait(self) -> Result<T, &'static str> {
        let (ref lock, ref cvar) = *self.result;
        let mut lock = lock.lock().unwrap();

        // 1. Check if the result is immediately available.
        match lock.take() {
            Some(FutureStatus::Returned(result)) => return Ok(result),
            Some(FutureStatus::Dropped) => return Err("The future was dropped"),
            None => {
                // `None` is fine here. It means `wait` was called
                // before a result could be returned from the lock.
            }
        }

        // 2. The result wasn't immediately available, so we have to wait.
        match cvar.wait(lock).unwrap().take() {
            Some(FutureStatus::Returned(result)) => Ok(result),
            Some(FutureStatus::Dropped) => Err("The future was dropped"),
            None => {
                // Shouldn't be possible
                unreachable!()
            }
        }
    }
}

/// Created from [`Future::await`]. Used to return a value to a [`Future`].
///
/// `FutureLock` is meant to be sent to a different thread than the one it was created on. Once on
/// a separate thread, work can be done and sent back to the original thread, using `FutureLock`.
///
/// A call to [`Future::wait`] will block until either [`send`] is called or the
/// `FutureLock` is dropped.
///
/// [`Future`] and `FutureLock` can be thought of as a one-time channel, where [`Future`] is the
/// receiver and `FutureLock` is the sender.
///
/// [`Future`]: struct.Future.html
/// [`Future::wait`]: struct.Future.html#method.wait
/// [`Future::await`]: struct.Future.html#method.await
/// [`send`]: #method.send
pub struct FutureLock<T> {
    value: Arc<(Mutex<Option<FutureStatus<T>>>, Condvar)>,
    has_responded: bool,
}

impl<T> FutureLock<T> {
    fn new(value: Arc<(Mutex<Option<FutureStatus<T>>>, Condvar)>) -> Self {
        FutureLock {
            value: value,
            has_responded: false,
        }
    }

    /// Consumes the lock and returns a value to the [`Future`](struct.Future.html) that was created with this lock.
    pub fn send(mut self, result: T) {
        self.has_responded = true;
        let (ref value, ref cvar) = *self.value;
        let mut value = value.lock().unwrap();
        *value = Some(FutureStatus::Returned(result));
        cvar.notify_one();
    }
}

impl<T> Drop for FutureLock<T> {
    fn drop(&mut self) {
        // If the `FutureLock` hasn't been used to send a result, it needs to signal that it's been
        // dropped or else the `Future` will `await` forever.
        if !self.has_responded {
            self.has_responded = true;
            let (ref value, ref cvar) = *self.value;
            let mut value = value.lock().unwrap();
            *value = Some(FutureStatus::Dropped);
            cvar.notify_one()
        }
    }
}
