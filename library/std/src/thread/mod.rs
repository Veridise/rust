//! Native threads.
//!
//! ## The threading model
//!
//! An executing Rust program consists of a collection of native OS threads,
//! each with their own stack and local state. Threads can be named, and
//! provide some built-in support for low-level synchronization.
//!
//! Communication between threads can be done through
//! [channels], Rust's message-passing types, along with [other forms of thread
//! synchronization](../../std/sync/index.html) and shared-memory data
//! structures. In particular, types that are guaranteed to be
//! threadsafe are easily shared between threads using the
//! atomically-reference-counted container, [`Arc`].
//!
//! Fatal logic errors in Rust cause *thread panic*, during which
//! a thread will unwind the stack, running destructors and freeing
//! owned resources. While not meant as a 'try/catch' mechanism, panics
//! in Rust can nonetheless be caught (unless compiling with `panic=abort`) with
//! [`catch_unwind`](../../std/panic/fn.catch_unwind.html) and recovered
//! from, or alternatively be resumed with
//! [`resume_unwind`](../../std/panic/fn.resume_unwind.html). If the panic
//! is not caught the thread will exit, but the panic may optionally be
//! detected from a different thread with [`join`]. If the main thread panics
//! without the panic being caught, the application will exit with a
//! non-zero exit code.
//!
//! When the main thread of a Rust program terminates, the entire program shuts
//! down, even if other threads are still running. However, this module provides
//! convenient facilities for automatically waiting for the termination of a
//! thread (i.e., join).
//!
//! ## Spawning a thread
//!
//! A new thread can be spawned using the [`thread::spawn`][`spawn`] function:
//!
//! ```rust
//! use std::thread;
//!
//! thread::spawn(move || {
//!     // some work here
//! });
//! ```
//!
//! In this example, the spawned thread is "detached," which means that there is
//! no way for the program to learn when the spawned thread completes or otherwise
//! terminates.
//!
//! To learn when a thread completes, it is necessary to capture the [`JoinHandle`]
//! object that is returned by the call to [`spawn`], which provides
//! a `join` method that allows the caller to wait for the completion of the
//! spawned thread:
//!
//! ```rust
//! use std::thread;
//!
//! let thread_join_handle = thread::spawn(move || {
//!     // some work here
//! });
//! // some work here
//! let res = thread_join_handle.join();
//! ```
//!
//! The [`join`] method returns a [`thread::Result`] containing [`Ok`] of the final
//! value produced by the spawned thread, or [`Err`] of the value given to
//! a call to [`panic!`] if the thread panicked.
//!
//! Note that there is no parent/child relationship between a thread that spawns a
//! new thread and the thread being spawned.  In particular, the spawned thread may or
//! may not outlive the spawning thread, unless the spawning thread is the main thread.
//!
//! ## Configuring threads
//!
//! A new thread can be configured before it is spawned via the [`Builder`] type,
//! which currently allows you to set the name and stack size for the thread:
//!
//! ```rust
//! # #![allow(unused_must_use)]
//! use std::thread;
//!
//! thread::Builder::new().name("thread1".to_string()).spawn(move || {
//!     println!("Hello, world!");
//! });
//! ```
//!
//! ## The `Thread` type
//!
//! Threads are represented via the [`Thread`] type, which you can get in one of
//! two ways:
//!
//! * By spawning a new thread, e.g., using the [`thread::spawn`][`spawn`]
//!   function, and calling [`thread`][`JoinHandle::thread`] on the [`JoinHandle`].
//! * By requesting the current thread, using the [`thread::current`] function.
//!
//! The [`thread::current`] function is available even for threads not spawned
//! by the APIs of this module.
//!
//! ## Thread-local storage
//!
//! This module also provides an implementation of thread-local storage for Rust
//! programs. Thread-local storage is a method of storing data into a global
//! variable that each thread in the program will have its own copy of.
//! Threads do not share this data, so accesses do not need to be synchronized.
//!
//! A thread-local key owns the value it contains and will destroy the value when the
//! thread exits. It is created with the [`thread_local!`] macro and can contain any
//! value that is `'static` (no borrowed pointers). It provides an accessor function,
//! [`with`], that yields a shared reference to the value to the specified
//! closure. Thread-local keys allow only shared access to values, as there would be no
//! way to guarantee uniqueness if mutable borrows were allowed. Most values
//! will want to make use of some form of **interior mutability** through the
//! [`Cell`] or [`RefCell`] types.
//!
//! ## Naming threads
//!
//! Threads are able to have associated names for identification purposes. By default, spawned
//! threads are unnamed. To specify a name for a thread, build the thread with [`Builder`] and pass
//! the desired thread name to [`Builder::name`]. To retrieve the thread name from within the
//! thread, use [`Thread::name`]. A couple of examples where the name of a thread gets used:
//!
//! * If a panic occurs in a named thread, the thread name will be printed in the panic message.
//! * The thread name is provided to the OS where applicable (e.g., `pthread_setname_np` in
//!   unix-like platforms).
//!
//! ## Stack size
//!
//! The default stack size is platform-dependent and subject to change.
//! Currently, it is 2 MiB on all Tier-1 platforms.
//!
//! There are two ways to manually specify the stack size for spawned threads:
//!
//! * Build the thread with [`Builder`] and pass the desired stack size to [`Builder::stack_size`].
//! * Set the `RUST_MIN_STACK` environment variable to an integer representing the desired stack
//!   size (in bytes). Note that setting [`Builder::stack_size`] will override this. Be aware that
//!   changes to `RUST_MIN_STACK` may be ignored after program start.
//!
//! Note that the stack size of the main thread is *not* determined by Rust.
//!
//! [channels]: crate::sync::mpsc
//! [`join`]: JoinHandle::join
//! [`Result`]: crate::result::Result
//! [`Ok`]: crate::result::Result::Ok
//! [`Err`]: crate::result::Result::Err
//! [`thread::current`]: current
//! [`thread::Result`]: Result
//! [`unpark`]: Thread::unpark
//! [`thread::park_timeout`]: park_timeout
//! [`Cell`]: crate::cell::Cell
//! [`RefCell`]: crate::cell::RefCell
//! [`with`]: LocalKey::with
//! [`thread_local!`]: crate::thread_local

#![stable(feature = "rust1", since = "1.0.0")]
#![deny(unsafe_op_in_unsafe_fn)]
// Under `test`, `__FastLocalKeyInner` seems unused.
#![cfg_attr(test, allow(dead_code))]

#[cfg(all(test, not(target_os = "emscripten")))]
mod tests;

use crate::any::Any;
use crate::cell::UnsafeCell;
use crate::ffi::{CStr, CString};
use crate::fmt;
use crate::io;
use crate::marker::PhantomData;
use crate::mem::{self, forget};
use crate::num::NonZeroU64;
use crate::num::NonZeroUsize;
use crate::panic;
use crate::panicking;
use crate::pin::Pin;
use crate::ptr::addr_of_mut;
use crate::str;
use crate::sync::Arc;
use crate::sys::thread as imp;
use crate::sys_common::thread;
use crate::sys_common::thread_info;
use crate::sys_common::thread_parking::Parker;
use crate::sys_common::{AsInner, IntoInner};
use crate::time::Duration;

#[stable(feature = "scoped_threads", since = "1.63.0")]
mod scoped;

#[stable(feature = "scoped_threads", since = "1.63.0")]
pub use scoped::{scope, Scope, ScopedJoinHandle};

////////////////////////////////////////////////////////////////////////////////
// Thread-local storage
////////////////////////////////////////////////////////////////////////////////

#[macro_use]
mod local;

#[stable(feature = "rust1", since = "1.0.0")]
pub use self::local::{AccessError, LocalKey};

// Provide the type used by the thread_local! macro to access TLS keys. This
// needs to be kept in sync with the macro itself (in `local.rs`).
// There are three types: "static", "fast", "OS". The "OS" thread local key
// type is accessed via platform-specific API calls and is slow, while the "fast"
// key type is accessed via code generated via LLVM, where TLS keys are set up
// by the elf linker. "static" is for single-threaded platforms where a global
// static is sufficient.

// Implementation details used by the thread_local!{} macro.
#[doc(hidden)]
#[unstable(feature = "thread_local_internals", issue = "none")]
pub mod local_impl {
    pub use crate::sys::common::thread_local::{thread_local_inner, Key};
}

////////////////////////////////////////////////////////////////////////////////
// Builder
////////////////////////////////////////////////////////////////////////////////

/// Thread factory, which can be used in order to configure the properties of
/// a new thread.
///
/// Methods can be chained on it in order to configure it.
///
/// The two configurations available are:
///
/// - [`name`]: specifies an [associated name for the thread][naming-threads]
/// - [`stack_size`]: specifies the [desired stack size for the thread][stack-size]
///
/// The [`spawn`] method will take ownership of the builder and create an
/// [`io::Result`] to the thread handle with the given configuration.
///
/// The [`thread::spawn`] free function uses a `Builder` with default
/// configuration and [`unwrap`]s its return value.
///
/// You may want to use [`spawn`] instead of [`thread::spawn`], when you want
/// to recover from a failure to launch a thread, indeed the free function will
/// panic where the `Builder` method will return a [`io::Result`].
///
/// # Examples
///
/// ```
/// use std::thread;
///
/// let builder = thread::Builder::new();
///
/// let handler = builder.spawn(|| {
///     // thread code
/// }).unwrap();
///
/// handler.join().unwrap();
/// ```
///
/// [`stack_size`]: Builder::stack_size
/// [`name`]: Builder::name
/// [`spawn`]: Builder::spawn
/// [`thread::spawn`]: spawn
/// [`io::Result`]: crate::io::Result
/// [`unwrap`]: crate::result::Result::unwrap
/// [naming-threads]: ./index.html#naming-threads
/// [stack-size]: ./index.html#stack-size
#[must_use = "must eventually spawn the thread"]
#[stable(feature = "rust1", since = "1.0.0")]
#[derive(Debug)]
pub struct Builder {
    // A name for the thread-to-be, for identification in panic messages
    name: Option<String>,
    // The size of the stack for the spawned thread in bytes
    stack_size: Option<usize>,
}

impl Builder {
    /// Generates the base configuration for spawning a thread, from which
    /// configuration methods can be chained.
    ///
    /// # Examples
    ///
    /// ```
    /// use std::thread;
    ///
    /// let builder = thread::Builder::new()
    ///                               .name("foo".into())
    ///                               .stack_size(32 * 1024);
    ///
    /// let handler = builder.spawn(|| {
    ///     // thread code
    /// }).unwrap();
    ///
    /// handler.join().unwrap();
    /// ```
    #[stable(feature = "rust1", since = "1.0.0")]
    pub fn new() -> Builder {
        Builder { name: None, stack_size: None }
    }

    /// Names the thread-to-be. Currently the name is used for identification
    /// only in panic messages.
    ///
    /// The name must not contain null bytes (`\0`).
    ///
    /// For more information about named threads, see
    /// [this module-level documentation][naming-threads].
    ///
    /// # Examples
    ///
    /// ```
    /// use std::thread;
    ///
    /// let builder = thread::Builder::new()
    ///     .name("foo".into());
    ///
    /// let handler = builder.spawn(|| {
    ///     assert_eq!(thread::current().name(), Some("foo"))
    /// }).unwrap();
    ///
    /// handler.join().unwrap();
    /// ```
    ///
    /// [naming-threads]: ./index.html#naming-threads
    #[stable(feature = "rust1", since = "1.0.0")]
    pub fn name(mut self, name: String) -> Builder {
        self.name = Some(name);
        self
    }

    /// Sets the size of the stack (in bytes) for the new thread.
    ///
    /// The actual stack size may be greater than this value if
    /// the platform specifies a minimal stack size.
    ///
    /// For more information about the stack size for threads, see
    /// [this module-level documentation][stack-size].
    ///
    /// # Examples
    ///
    /// ```
    /// use std::thread;
    ///
    /// let builder = thread::Builder::new().stack_size(32 * 1024);
    /// ```
    ///
    /// [stack-size]: ./index.html#stack-size
    #[stable(feature = "rust1", since = "1.0.0")]
    pub fn stack_size(mut self, size: usize) -> Builder {
        self.stack_size = Some(size);
        self
    }

    /// Spawns a new thread by taking ownership of the `Builder`, and returns an
    /// [`io::Result`] to its [`JoinHandle`].
    ///
    /// The spawned thread may outlive the caller (unless the caller thread
    /// is the main thread; the whole process is terminated when the main
    /// thread finishes). The join handle can be used to block on
    /// termination of the spawned thread, including recovering its panics.
    ///
    /// For a more complete documentation see [`thread::spawn`][`spawn`].
    ///
    /// # Errors
    ///
    /// Unlike the [`spawn`] free function, this method yields an
    /// [`io::Result`] to capture any failure to create the thread at
    /// the OS level.
    ///
    /// [`io::Result`]: crate::io::Result
    ///
    /// # Panics
    ///
    /// Panics if a thread name was set and it contained null bytes.
    ///
    /// # Examples
    ///
    /// ```
    /// use std::thread;
    ///
    /// let builder = thread::Builder::new();
    ///
    /// let handler = builder.spawn(|| {
    ///     // thread code
    /// }).unwrap();
    ///
    /// handler.join().unwrap();
    /// ```
    #[stable(feature = "rust1", since = "1.0.0")]
    pub fn spawn<F, T>(self, f: F) -> io::Result<JoinHandle<T>>
    where
        F: FnOnce() -> T,
        F: Send + 'static,
        T: Send + 'static,
    {
        unsafe { self.spawn_unchecked(f) }
    }

    /// Spawns a new thread without any lifetime restrictions by taking ownership
    /// of the `Builder`, and returns an [`io::Result`] to its [`JoinHandle`].
    ///
    /// The spawned thread may outlive the caller (unless the caller thread
    /// is the main thread; the whole process is terminated when the main
    /// thread finishes). The join handle can be used to block on
    /// termination of the spawned thread, including recovering its panics.
    ///
    /// This method is identical to [`thread::Builder::spawn`][`Builder::spawn`],
    /// except for the relaxed lifetime bounds, which render it unsafe.
    /// For a more complete documentation see [`thread::spawn`][`spawn`].
    ///
    /// # Errors
    ///
    /// Unlike the [`spawn`] free function, this method yields an
    /// [`io::Result`] to capture any failure to create the thread at
    /// the OS level.
    ///
    /// # Panics
    ///
    /// Panics if a thread name was set and it contained null bytes.
    ///
    /// # Safety
    ///
    /// The caller has to ensure that the spawned thread does not outlive any
    /// references in the supplied thread closure and its return type.
    /// This can be guaranteed in two ways:
    ///
    /// - ensure that [`join`][`JoinHandle::join`] is called before any referenced
    /// data is dropped
    /// - use only types with `'static` lifetime bounds, i.e., those with no or only
    /// `'static` references (both [`thread::Builder::spawn`][`Builder::spawn`]
    /// and [`thread::spawn`][`spawn`] enforce this property statically)
    ///
    /// # Examples
    ///
    /// ```
    /// #![feature(thread_spawn_unchecked)]
    /// use std::thread;
    ///
    /// let builder = thread::Builder::new();
    ///
    /// let x = 1;
    /// let thread_x = &x;
    ///
    /// let handler = unsafe {
    ///     builder.spawn_unchecked(move || {
    ///         println!("x = {}", *thread_x);
    ///     }).unwrap()
    /// };
    ///
    /// // caller has to ensure `join()` is called, otherwise
    /// // it is possible to access freed memory if `x` gets
    /// // dropped before the thread closure is executed!
    /// handler.join().unwrap();
    /// ```
    ///
    /// [`io::Result`]: crate::io::Result
    #[unstable(feature = "thread_spawn_unchecked", issue = "55132")]
    pub unsafe fn spawn_unchecked<'a, F, T>(self, f: F) -> io::Result<JoinHandle<T>>
    where
        F: FnOnce() -> T,
        F: Send + 'a,
        T: Send + 'a,
    {
        Ok(JoinHandle(unsafe { self.spawn_unchecked_(f, None) }?))
    }

    unsafe fn spawn_unchecked_<'a, 'scope, F, T>(
        self,
        f: F,
        scope_data: Option<Arc<scoped::ScopeData>>,
    ) -> io::Result<JoinInner<'scope, T>>
    where
        F: FnOnce() -> T,
        F: Send + 'a,
        T: Send + 'a,
        'scope: 'a,
    {
        let Builder { name, stack_size } = self;

        let stack_size = stack_size.unwrap_or_else(thread::min_stack);

        let my_thread = Thread::new(name.map(|name| {
            CString::new(name).expect("thread name may not contain interior null bytes")
        }));
        let their_thread = my_thread.clone();

        let my_packet: Arc<Packet<'scope, T>> = Arc::new(Packet {
            scope: scope_data,
            result: UnsafeCell::new(None),
            _marker: PhantomData,
        });
        let their_packet = my_packet.clone();

        let output_capture = crate::io::set_output_capture(None);
        crate::io::set_output_capture(output_capture.clone());

        // Pass `f` in `MaybeUninit` because actually that closure might *run longer than the lifetime of `F`*.
        // See <https://github.com/rust-lang/rust/issues/101983> for more details.
        // To prevent leaks we use a wrapper that drops its contents.
        #[repr(transparent)]
        struct MaybeDangling<T>(mem::MaybeUninit<T>);
        impl<T> MaybeDangling<T> {
            fn new(x: T) -> Self {
                MaybeDangling(mem::MaybeUninit::new(x))
            }
            fn into_inner(self) -> T {
                // SAFETY: we are always initialized.
                let ret = unsafe { self.0.assume_init_read() };
                // Make sure we don't drop.
                mem::forget(self);
                ret
            }
        }
        impl<T> Drop for MaybeDangling<T> {
            fn drop(&mut self) {
                // SAFETY: we are always initialized.
                unsafe { self.0.assume_init_drop() };
            }
        }

        let f = MaybeDangling::new(f);
        let main = move || {
            if let Some(name) = their_thread.cname() {
                imp::Thread::set_name(name);
            }

            crate::io::set_output_capture(output_capture);

            // SAFETY: we constructed `f` initialized.
            let f = f.into_inner();
            // SAFETY: the stack guard passed is the one for the current thread.
            // This means the current thread's stack and the new thread's stack
            // are properly set and protected from each other.
            thread_info::set(unsafe { imp::guard::current() }, their_thread);
            let try_result = panic::catch_unwind(panic::AssertUnwindSafe(|| {
                crate::sys_common::backtrace::__rust_begin_short_backtrace(f)
            }));
            // SAFETY: `their_packet` as been built just above and moved by the
            // closure (it is an Arc<...>) and `my_packet` will be stored in the
            // same `JoinInner` as this closure meaning the mutation will be
            // safe (not modify it and affect a value far away).
            unsafe { *their_packet.result.get() = Some(try_result) };
            // Here `their_packet` gets dropped, and if this is the last `Arc` for that packet that
            // will call `decrement_num_running_threads` and therefore signal that this thread is
            // done.
            drop(their_packet);
            // Here, the lifetime `'a` and even `'scope` can end. `main` keeps running for a bit
            // after that before returning itself.
        };

        if let Some(scope_data) = &my_packet.scope {
            scope_data.increment_num_running_threads();
        }

        Ok(JoinInner {
            // SAFETY:
            //
            // `imp::Thread::new` takes a closure with a `'static` lifetime, since it's passed
            // through FFI or otherwise used with low-level threading primitives that have no
            // notion of or way to enforce lifetimes.
            //
            // As mentioned in the `Safety` section of this function's documentation, the caller of
            // this function needs to guarantee that the passed-in lifetime is sufficiently long
            // for the lifetime of the thread.
            //
            // Similarly, the `sys` implementation must guarantee that no references to the closure
            // exist after the thread has terminated, which is signaled by `Thread::join`
            // returning.
            native: unsafe {
                imp::Thread::new(
                    stack_size,
                    mem::transmute::<Box<dyn FnOnce() + 'a>, Box<dyn FnOnce() + 'static>>(
                        Box::new(main),
                    ),
                )?
            },
            thread: my_thread,
            packet: my_packet,
        })
    }
}

////////////////////////////////////////////////////////////////////////////////
// Free functions
////////////////////////////////////////////////////////////////////////////////

/// Spawns a new thread, returning a [`JoinHandle`] for it.
///
/// The join handle provides a [`join`] method that can be used to join the spawned
/// thread. If the spawned thread panics, [`join`] will return an [`Err`] containing
/// the argument given to [`panic!`].
///
/// If the join handle is dropped, the spawned thread will implicitly be *detached*.
/// In this case, the spawned thread may no longer be joined.
/// (It is the responsibility of the program to either eventually join threads it
/// creates or detach them; otherwise, a resource leak will result.)
///
/// This call will create a thread using default parameters of [`Builder`], if you
/// want to specify the stack size or the name of the thread, use this API
/// instead.
///
/// As you can see in the signature of `spawn` there are two constraints on
/// both the closure given to `spawn` and its return value, let's explain them:
///
/// - The `'static` constraint means that the closure and its return value
///   must have a lifetime of the whole program execution. The reason for this
///   is that threads can outlive the lifetime they have been created in.
///
///   Indeed if the thread, and by extension its return value, can outlive their
///   caller, we need to make sure that they will be valid afterwards, and since
///   we *can't* know when it will return we need to have them valid as long as
///   possible, that is until the end of the program, hence the `'static`
///   lifetime.
/// - The [`Send`] constraint is because the closure will need to be passed
///   *by value* from the thread where it is spawned to the new thread. Its
///   return value will need to be passed from the new thread to the thread
///   where it is `join`ed.
///   As a reminder, the [`Send`] marker trait expresses that it is safe to be
///   passed from thread to thread. [`Sync`] expresses that it is safe to have a
///   reference be passed from thread to thread.
///
/// # Panics
///
/// Panics if the OS fails to create a thread; use [`Builder::spawn`]
/// to recover from such errors.
///
/// # Examples
///
/// Creating a thread.
///
/// ```
/// use std::thread;
///
/// let handler = thread::spawn(|| {
///     // thread code
/// });
///
/// handler.join().unwrap();
/// ```
///
/// As mentioned in the module documentation, threads are usually made to
/// communicate using [`channels`], here is how it usually looks.
///
/// This example also shows how to use `move`, in order to give ownership
/// of values to a thread.
///
/// ```
/// use std::thread;
/// use std::sync::mpsc::channel;
///
/// let (tx, rx) = channel();
///
/// let sender = thread::spawn(move || {
///     tx.send("Hello, thread".to_owned())
///         .expect("Unable to send on channel");
/// });
///
/// let receiver = thread::spawn(move || {
///     let value = rx.recv().expect("Unable to receive from channel");
///     println!("{value}");
/// });
///
/// sender.join().expect("The sender thread has panicked");
/// receiver.join().expect("The receiver thread has panicked");
/// ```
///
/// A thread can also return a value through its [`JoinHandle`], you can use
/// this to make asynchronous computations (futures might be more appropriate
/// though).
///
/// ```
/// use std::thread;
///
/// let computation = thread::spawn(|| {
///     // Some expensive computation.
///     42
/// });
///
/// let result = computation.join().unwrap();
/// println!("{result}");
/// ```
///
/// [`channels`]: crate::sync::mpsc
/// [`join`]: JoinHandle::join
/// [`Err`]: crate::result::Result::Err
#[stable(feature = "rust1", since = "1.0.0")]
pub fn spawn<F, T>(f: F) -> JoinHandle<T>
where
    F: FnOnce() -> T,
    F: Send + 'static,
    T: Send + 'static,
{
    Builder::new().spawn(f).expect("failed to spawn thread")
}

/// Gets a handle to the thread that invokes it.
///
/// # Examples
///
/// Getting a handle to the current thread with `thread::current()`:
///
/// ```
/// use std::thread;
///
/// let handler = thread::Builder::new()
///     .name("named thread".into())
///     .spawn(|| {
///         let handle = thread::current();
///         assert_eq!(handle.name(), Some("named thread"));
///     })
///     .unwrap();
///
/// handler.join().unwrap();
/// ```
#[must_use]
#[stable(feature = "rust1", since = "1.0.0")]
pub fn current() -> Thread {
    thread_info::current_thread().expect(
        "use of std::thread::current() is not possible \
         after the thread's local data has been destroyed",
    )
}

/// Cooperatively gives up a timeslice to the OS scheduler.
///
/// This calls the underlying OS scheduler's yield primitive, signaling
/// that the calling thread is willing to give up its remaining timeslice
/// so that the OS may schedule other threads on the CPU.
///
/// A drawback of yielding in a loop is that if the OS does not have any
/// other ready threads to run on the current CPU, the thread will effectively
/// busy-wait, which wastes CPU time and energy.
///
/// Therefore, when waiting for events of interest, a programmer's first
/// choice should be to use synchronization devices such as [`channel`]s,
/// [`Condvar`]s, [`Mutex`]es or [`join`] since these primitives are
/// implemented in a blocking manner, giving up the CPU until the event
/// of interest has occurred which avoids repeated yielding.
///
/// `yield_now` should thus be used only rarely, mostly in situations where
/// repeated polling is required because there is no other suitable way to
/// learn when an event of interest has occurred.
///
/// # Examples
///
/// ```
/// use std::thread;
///
/// thread::yield_now();
/// ```
///
/// [`channel`]: crate::sync::mpsc
/// [`join`]: JoinHandle::join
/// [`Condvar`]: crate::sync::Condvar
/// [`Mutex`]: crate::sync::Mutex
#[stable(feature = "rust1", since = "1.0.0")]
pub fn yield_now() {
    imp::Thread::yield_now()
}

/// Determines whether the current thread is unwinding because of panic.
///
/// A common use of this feature is to poison shared resources when writing
/// unsafe code, by checking `panicking` when the `drop` is called.
///
/// This is usually not needed when writing safe code, as [`Mutex`es][Mutex]
/// already poison themselves when a thread panics while holding the lock.
///
/// This can also be used in multithreaded applications, in order to send a
/// message to other threads warning that a thread has panicked (e.g., for
/// monitoring purposes).
///
/// # Examples
///
/// ```should_panic
/// use std::thread;
///
/// struct SomeStruct;
///
/// impl Drop for SomeStruct {
///     fn drop(&mut self) {
///         if thread::panicking() {
///             println!("dropped while unwinding");
///         } else {
///             println!("dropped while not unwinding");
///         }
///     }
/// }
///
/// {
///     print!("a: ");
///     let a = SomeStruct;
/// }
///
/// {
///     print!("b: ");
///     let b = SomeStruct;
///     panic!()
/// }
/// ```
///
/// [Mutex]: crate::sync::Mutex
#[inline]
#[must_use]
#[stable(feature = "rust1", since = "1.0.0")]
pub fn panicking() -> bool {
    panicking::panicking()
}

/// Use [`sleep`].
///
/// Puts the current thread to sleep for at least the specified amount of time.
///
/// The thread may sleep longer than the duration specified due to scheduling
/// specifics or platform-dependent functionality. It will never sleep less.
///
/// This function is blocking, and should not be used in `async` functions.
///
/// # Platform-specific behavior
///
/// On Unix platforms, the underlying syscall may be interrupted by a
/// spurious wakeup or signal handler. To ensure the sleep occurs for at least
/// the specified duration, this function may invoke that system call multiple
/// times.
///
/// # Examples
///
/// ```no_run
/// use std::thread;
///
/// // Let's sleep for 2 seconds:
/// thread::sleep_ms(2000);
/// ```
#[stable(feature = "rust1", since = "1.0.0")]
#[deprecated(since = "1.6.0", note = "replaced by `std::thread::sleep`")]
pub fn sleep_ms(ms: u32) {
    sleep(Duration::from_millis(ms as u64))
}

/// Puts the current thread to sleep for at least the specified amount of time.
///
/// The thread may sleep longer than the duration specified due to scheduling
/// specifics or platform-dependent functionality. It will never sleep less.
///
/// This function is blocking, and should not be used in `async` functions.
///
/// # Platform-specific behavior
///
/// On Unix platforms, the underlying syscall may be interrupted by a
/// spurious wakeup or signal handler. To ensure the sleep occurs for at least
/// the specified duration, this function may invoke that system call multiple
/// times.
/// Platforms which do not support nanosecond precision for sleeping will
/// have `dur` rounded up to the nearest granularity of time they can sleep for.
///
/// Currently, specifying a zero duration on Unix platforms returns immediately
/// without invoking the underlying [`nanosleep`] syscall, whereas on Windows
/// platforms the underlying [`Sleep`] syscall is always invoked.
/// If the intention is to yield the current time-slice you may want to use
/// [`yield_now`] instead.
///
/// [`nanosleep`]: https://linux.die.net/man/2/nanosleep
/// [`Sleep`]: https://docs.microsoft.com/en-us/windows/win32/api/synchapi/nf-synchapi-sleep
///
/// # Examples
///
/// ```no_run
/// use std::{thread, time};
///
/// let ten_millis = time::Duration::from_millis(10);
/// let now = time::Instant::now();
///
/// thread::sleep(ten_millis);
///
/// assert!(now.elapsed() >= ten_millis);
/// ```
#[stable(feature = "thread_sleep", since = "1.4.0")]
pub fn sleep(dur: Duration) {
    imp::Thread::sleep(dur)
}

/// Used to ensure that `park` and `park_timeout` do not unwind, as that can
/// cause undefined behaviour if not handled correctly (see #102398 for context).
struct PanicGuard;

impl Drop for PanicGuard {
    fn drop(&mut self) {
        rtabort!("an irrecoverable error occurred while synchronizing threads")
    }
}

/// Blocks unless or until the current thread's token is made available.
///
/// A call to `park` does not guarantee that the thread will remain parked
/// forever, and callers should be prepared for this possibility. However,
/// it is guaranteed that this function will not panic (it may abort the
/// process if the implementation encounters some rare errors).
///
/// # park and unpark
///
/// Every thread is equipped with some basic low-level blocking support, via the
/// [`thread::park`][`park`] function and [`thread::Thread::unpark`][`unpark`]
/// method. [`park`] blocks the current thread, which can then be resumed from
/// another thread by calling the [`unpark`] method on the blocked thread's
/// handle.
///
/// Conceptually, each [`Thread`] handle has an associated token, which is
/// initially not present:
///
/// * The [`thread::park`][`park`] function blocks the current thread unless or
///   until the token is available for its thread handle, at which point it
///   atomically consumes the token. It may also return *spuriously*, without
///   consuming the token. [`thread::park_timeout`] does the same, but allows
///   specifying a maximum time to block the thread for.
///
/// * The [`unpark`] method on a [`Thread`] atomically makes the token available
///   if it wasn't already. Because the token is initially absent, [`unpark`]
///   followed by [`park`] will result in the second call returning immediately.
///
/// In other words, each [`Thread`] acts a bit like a spinlock that can be
/// locked and unlocked using `park` and `unpark`.
///
/// Notice that being unblocked does not imply any synchronization with someone
/// that unparked this thread, it could also be spurious.
/// For example, it would be a valid, but inefficient, implementation to make both [`park`] and
/// [`unpark`] return immediately without doing anything.
///
/// The API is typically used by acquiring a handle to the current thread,
/// placing that handle in a shared data structure so that other threads can
/// find it, and then `park`ing in a loop. When some desired condition is met, another
/// thread calls [`unpark`] on the handle.
///
/// The motivation for this design is twofold:
///
/// * It avoids the need to allocate mutexes and condvars when building new
///   synchronization primitives; the threads already provide basic
///   blocking/signaling.
///
/// * It can be implemented very efficiently on many platforms.
///
/// # Examples
///
/// ```
/// use std::thread;
/// use std::sync::{Arc, atomic::{Ordering, AtomicBool}};
/// use std::time::Duration;
///
/// let flag = Arc::new(AtomicBool::new(false));
/// let flag2 = Arc::clone(&flag);
///
/// let parked_thread = thread::spawn(move || {
///     // We want to wait until the flag is set. We *could* just spin, but using
///     // park/unpark is more efficient.
///     while !flag2.load(Ordering::Acquire) {
///         println!("Parking thread");
///         thread::park();
///         // We *could* get here spuriously, i.e., way before the 10ms below are over!
///         // But that is no problem, we are in a loop until the flag is set anyway.
///         println!("Thread unparked");
///     }
///     println!("Flag received");
/// });
///
/// // Let some time pass for the thread to be spawned.
/// thread::sleep(Duration::from_millis(10));
///
/// // Set the flag, and let the thread wake up.
/// // There is no race condition here, if `unpark`
/// // happens first, `park` will return immediately.
/// // Hence there is no risk of a deadlock.
/// flag.store(true, Ordering::Release);
/// println!("Unpark the thread");
/// parked_thread.thread().unpark();
///
/// parked_thread.join().unwrap();
/// ```
///
/// [`unpark`]: Thread::unpark
/// [`thread::park_timeout`]: park_timeout
#[stable(feature = "rust1", since = "1.0.0")]
pub fn park() {
    let guard = PanicGuard;
    // SAFETY: park_timeout is called on the parker owned by this thread.
    unsafe {
        current().inner.as_ref().parker().park();
    }
    // No panic occurred, do not abort.
    forget(guard);
}

/// Use [`park_timeout`].
///
/// Blocks unless or until the current thread's token is made available or
/// the specified duration has been reached (may wake spuriously).
///
/// The semantics of this function are equivalent to [`park`] except
/// that the thread will be blocked for roughly no longer than `dur`. This
/// method should not be used for precise timing due to anomalies such as
/// preemption or platform differences that might not cause the maximum
/// amount of time waited to be precisely `ms` long.
///
/// See the [park documentation][`park`] for more detail.
#[stable(feature = "rust1", since = "1.0.0")]
#[deprecated(since = "1.6.0", note = "replaced by `std::thread::park_timeout`")]
pub fn park_timeout_ms(ms: u32) {
    park_timeout(Duration::from_millis(ms as u64))
}

/// Blocks unless or until the current thread's token is made available or
/// the specified duration has been reached (may wake spuriously).
///
/// The semantics of this function are equivalent to [`park`][park] except
/// that the thread will be blocked for roughly no longer than `dur`. This
/// method should not be used for precise timing due to anomalies such as
/// preemption or platform differences that might not cause the maximum
/// amount of time waited to be precisely `dur` long.
///
/// See the [park documentation][park] for more details.
///
/// # Platform-specific behavior
///
/// Platforms which do not support nanosecond precision for sleeping will have
/// `dur` rounded up to the nearest granularity of time they can sleep for.
///
/// # Examples
///
/// Waiting for the complete expiration of the timeout:
///
/// ```rust,no_run
/// use std::thread::park_timeout;
/// use std::time::{Instant, Duration};
///
/// let timeout = Duration::from_secs(2);
/// let beginning_park = Instant::now();
///
/// let mut timeout_remaining = timeout;
/// loop {
///     park_timeout(timeout_remaining);
///     let elapsed = beginning_park.elapsed();
///     if elapsed >= timeout {
///         break;
///     }
///     println!("restarting park_timeout after {elapsed:?}");
///     timeout_remaining = timeout - elapsed;
/// }
/// ```
#[stable(feature = "park_timeout", since = "1.4.0")]
pub fn park_timeout(dur: Duration) {
    let guard = PanicGuard;
    // SAFETY: park_timeout is called on the parker owned by this thread.
    unsafe {
        current().inner.as_ref().parker().park_timeout(dur);
    }
    // No panic occurred, do not abort.
    forget(guard);
}

////////////////////////////////////////////////////////////////////////////////
// ThreadId
////////////////////////////////////////////////////////////////////////////////

/// A unique identifier for a running thread.
///
/// A `ThreadId` is an opaque object that uniquely identifies each thread
/// created during the lifetime of a process. `ThreadId`s are guaranteed not to
/// be reused, even when a thread terminates. `ThreadId`s are under the control
/// of Rust's standard library and there may not be any relationship between
/// `ThreadId` and the underlying platform's notion of a thread identifier --
/// the two concepts cannot, therefore, be used interchangeably. A `ThreadId`
/// can be retrieved from the [`id`] method on a [`Thread`].
///
/// # Examples
///
/// ```
/// use std::thread;
///
/// let other_thread = thread::spawn(|| {
///     thread::current().id()
/// });
///
/// let other_thread_id = other_thread.join().unwrap();
/// assert!(thread::current().id() != other_thread_id);
/// ```
///
/// [`id`]: Thread::id
#[stable(feature = "thread_id", since = "1.19.0")]
#[derive(Eq, PartialEq, Clone, Copy, Hash, Debug)]
pub struct ThreadId(NonZeroU64);

impl ThreadId {
    // Generate a new unique thread ID.
    fn new() -> ThreadId {
        #[cold]
        fn exhausted() -> ! {
            panic!("failed to generate unique thread ID: bitspace exhausted")
        }

        cfg_if::cfg_if! {
            if #[cfg(target_has_atomic = "64")] {
                use crate::sync::atomic::{AtomicU64, Ordering::Relaxed};

                static COUNTER: AtomicU64 = AtomicU64::new(0);

                let mut last = COUNTER.load(Relaxed);
                loop {
                    let Some(id) = last.checked_add(1) else {
                        exhausted();
                    };

                    match COUNTER.compare_exchange_weak(last, id, Relaxed, Relaxed) {
                        Ok(_) => return ThreadId(NonZeroU64::new(id).unwrap()),
                        Err(id) => last = id,
                    }
                }
            } else {
                use crate::sync::{Mutex, PoisonError};

                static COUNTER: Mutex<u64> = Mutex::new(0);

                let mut counter = COUNTER.lock().unwrap_or_else(PoisonError::into_inner);
                let Some(id) = counter.checked_add(1) else {
                    // in case the panic handler ends up calling `ThreadId::new()`,
                    // avoid reentrant lock acquire.
                    drop(counter);
                    exhausted();
                };

                *counter = id;
                drop(counter);
                ThreadId(NonZeroU64::new(id).unwrap())
            }
        }
    }

    /// This returns a numeric identifier for the thread identified by this
    /// `ThreadId`.
    ///
    /// As noted in the documentation for the type itself, it is essentially an
    /// opaque ID, but is guaranteed to be unique for each thread. The returned
    /// value is entirely opaque -- only equality testing is stable. Note that
    /// it is not guaranteed which values new threads will return, and this may
    /// change across Rust versions.
    #[must_use]
    #[unstable(feature = "thread_id_value", issue = "67939")]
    pub fn as_u64(&self) -> NonZeroU64 {
        self.0
    }
}

////////////////////////////////////////////////////////////////////////////////
// Thread
////////////////////////////////////////////////////////////////////////////////

/// The internal representation of a `Thread` handle
struct Inner {
    name: Option<CString>, // Guaranteed to be UTF-8
    id: ThreadId,
    parker: Parker,
}

impl Inner {
    fn parker(self: Pin<&Self>) -> Pin<&Parker> {
        unsafe { Pin::map_unchecked(self, |inner| &inner.parker) }
    }
}

#[derive(Clone)]
#[stable(feature = "rust1", since = "1.0.0")]
/// A handle to a thread.
///
/// Threads are represented via the `Thread` type, which you can get in one of
/// two ways:
///
/// * By spawning a new thread, e.g., using the [`thread::spawn`][`spawn`]
///   function, and calling [`thread`][`JoinHandle::thread`] on the
///   [`JoinHandle`].
/// * By requesting the current thread, using the [`thread::current`] function.
///
/// The [`thread::current`] function is available even for threads not spawned
/// by the APIs of this module.
///
/// There is usually no need to create a `Thread` struct yourself, one
/// should instead use a function like `spawn` to create new threads, see the
/// docs of [`Builder`] and [`spawn`] for more details.
///
/// [`thread::current`]: current
pub struct Thread {
    inner: Pin<Arc<Inner>>,
}

impl Thread {
    // Used only internally to construct a thread object without spawning
    // Panics if the name contains nuls.
    pub(crate) fn new(name: Option<CString>) -> Thread {
        // We have to use `unsafe` here to construct the `Parker` in-place,
        // which is required for the UNIX implementation.
        //
        // SAFETY: We pin the Arc immediately after creation, so its address never
        // changes.
        let inner = unsafe {
            let mut arc = Arc::<Inner>::new_uninit();
            let ptr = Arc::get_mut_unchecked(&mut arc).as_mut_ptr();
            addr_of_mut!((*ptr).name).write(name);
            addr_of_mut!((*ptr).id).write(ThreadId::new());
            Parker::new_in_place(addr_of_mut!((*ptr).parker));
            Pin::new_unchecked(arc.assume_init())
        };

        Thread { inner }
    }

    /// Atomically makes the handle's token available if it is not already.
    ///
    /// Every thread is equipped with some basic low-level blocking support, via
    /// the [`park`][park] function and the `unpark()` method. These can be
    /// used as a more CPU-efficient implementation of a spinlock.
    ///
    /// See the [park documentation][park] for more details.
    ///
    /// # Examples
    ///
    /// ```
    /// use std::thread;
    /// use std::time::Duration;
    ///
    /// let parked_thread = thread::Builder::new()
    ///     .spawn(|| {
    ///         println!("Parking thread");
    ///         thread::park();
    ///         println!("Thread unparked");
    ///     })
    ///     .unwrap();
    ///
    /// // Let some time pass for the thread to be spawned.
    /// thread::sleep(Duration::from_millis(10));
    ///
    /// println!("Unpark the thread");
    /// parked_thread.thread().unpark();
    ///
    /// parked_thread.join().unwrap();
    /// ```
    #[stable(feature = "rust1", since = "1.0.0")]
    #[inline]
    pub fn unpark(&self) {
        self.inner.as_ref().parker().unpark();
    }

    /// Gets the thread's unique identifier.
    ///
    /// # Examples
    ///
    /// ```
    /// use std::thread;
    ///
    /// let other_thread = thread::spawn(|| {
    ///     thread::current().id()
    /// });
    ///
    /// let other_thread_id = other_thread.join().unwrap();
    /// assert!(thread::current().id() != other_thread_id);
    /// ```
    #[stable(feature = "thread_id", since = "1.19.0")]
    #[must_use]
    pub fn id(&self) -> ThreadId {
        self.inner.id
    }

    /// Gets the thread's name.
    ///
    /// For more information about named threads, see
    /// [this module-level documentation][naming-threads].
    ///
    /// # Examples
    ///
    /// Threads by default have no name specified:
    ///
    /// ```
    /// use std::thread;
    ///
    /// let builder = thread::Builder::new();
    ///
    /// let handler = builder.spawn(|| {
    ///     assert!(thread::current().name().is_none());
    /// }).unwrap();
    ///
    /// handler.join().unwrap();
    /// ```
    ///
    /// Thread with a specified name:
    ///
    /// ```
    /// use std::thread;
    ///
    /// let builder = thread::Builder::new()
    ///     .name("foo".into());
    ///
    /// let handler = builder.spawn(|| {
    ///     assert_eq!(thread::current().name(), Some("foo"))
    /// }).unwrap();
    ///
    /// handler.join().unwrap();
    /// ```
    ///
    /// [naming-threads]: ./index.html#naming-threads
    #[stable(feature = "rust1", since = "1.0.0")]
    #[must_use]
    pub fn name(&self) -> Option<&str> {
        self.cname().map(|s| unsafe { str::from_utf8_unchecked(s.to_bytes()) })
    }

    fn cname(&self) -> Option<&CStr> {
        self.inner.name.as_deref()
    }
}

#[stable(feature = "rust1", since = "1.0.0")]
impl fmt::Debug for Thread {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Thread")
            .field("id", &self.id())
            .field("name", &self.name())
            .finish_non_exhaustive()
    }
}

////////////////////////////////////////////////////////////////////////////////
// JoinHandle
////////////////////////////////////////////////////////////////////////////////

/// A specialized [`Result`] type for threads.
///
/// Indicates the manner in which a thread exited.
///
/// The value contained in the `Result::Err` variant
/// is the value the thread panicked with;
/// that is, the argument the `panic!` macro was called with.
/// Unlike with normal errors, this value doesn't implement
/// the [`Error`](crate::error::Error) trait.
///
/// Thus, a sensible way to handle a thread panic is to either:
///
/// 1. propagate the panic with [`std::panic::resume_unwind`]
/// 2. or in case the thread is intended to be a subsystem boundary
/// that is supposed to isolate system-level failures,
/// match on the `Err` variant and handle the panic in an appropriate way
///
/// A thread that completes without panicking is considered to exit successfully.
///
/// # Examples
///
/// Matching on the result of a joined thread:
///
/// ```no_run
/// use std::{fs, thread, panic};
///
/// fn copy_in_thread() -> thread::Result<()> {
///     thread::spawn(|| {
///         fs::copy("foo.txt", "bar.txt").unwrap();
///     }).join()
/// }
///
/// fn main() {
///     match copy_in_thread() {
///         Ok(_) => println!("copy succeeded"),
///         Err(e) => panic::resume_unwind(e),
///     }
/// }
/// ```
///
/// [`Result`]: crate::result::Result
/// [`std::panic::resume_unwind`]: crate::panic::resume_unwind
#[stable(feature = "rust1", since = "1.0.0")]
pub type Result<T> = crate::result::Result<T, Box<dyn Any + Send + 'static>>;

// This packet is used to communicate the return value between the spawned
// thread and the rest of the program. It is shared through an `Arc` and
// there's no need for a mutex here because synchronization happens with `join()`
// (the caller will never read this packet until the thread has exited).
//
// An Arc to the packet is stored into a `JoinInner` which in turns is placed
// in `JoinHandle`.
struct Packet<'scope, T> {
    scope: Option<Arc<scoped::ScopeData>>,
    result: UnsafeCell<Option<Result<T>>>,
    _marker: PhantomData<Option<&'scope scoped::ScopeData>>,
}

// Due to the usage of `UnsafeCell` we need to manually implement Sync.
// The type `T` should already always be Send (otherwise the thread could not
// have been created) and the Packet is Sync because all access to the
// `UnsafeCell` synchronized (by the `join()` boundary), and `ScopeData` is Sync.
unsafe impl<'scope, T: Sync> Sync for Packet<'scope, T> {}

impl<'scope, T> Drop for Packet<'scope, T> {
    fn drop(&mut self) {
        // If this packet was for a thread that ran in a scope, the thread
        // panicked, and nobody consumed the panic payload, we make sure
        // the scope function will panic.
        let unhandled_panic = matches!(self.result.get_mut(), Some(Err(_)));
        // Drop the result without causing unwinding.
        // This is only relevant for threads that aren't join()ed, as
        // join() will take the `result` and set it to None, such that
        // there is nothing left to drop here.
        // If this panics, we should handle that, because we're outside the
        // outermost `catch_unwind` of our thread.
        // We just abort in that case, since there's nothing else we can do.
        // (And even if we tried to handle it somehow, we'd also need to handle
        // the case where the panic payload we get out of it also panics on
        // drop, and so on. See issue #86027.)
        if let Err(_) = panic::catch_unwind(panic::AssertUnwindSafe(|| {
            *self.result.get_mut() = None;
        })) {
            rtabort!("thread result panicked on drop");
        }
        // Book-keeping so the scope knows when it's done.
        if let Some(scope) = &self.scope {
            // Now that there will be no more user code running on this thread
            // that can use 'scope, mark the thread as 'finished'.
            // It's important we only do this after the `result` has been dropped,
            // since dropping it might still use things it borrowed from 'scope.
            scope.decrement_num_running_threads(unhandled_panic);
        }
    }
}

/// Inner representation for JoinHandle
struct JoinInner<'scope, T> {
    native: imp::Thread,
    thread: Thread,
    packet: Arc<Packet<'scope, T>>,
}

impl<'scope, T> JoinInner<'scope, T> {
    fn join(mut self) -> Result<T> {
        self.native.join();
        Arc::get_mut(&mut self.packet).unwrap().result.get_mut().take().unwrap()
    }
}

/// An owned permission to join on a thread (block on its termination).
///
/// A `JoinHandle` *detaches* the associated thread when it is dropped, which
/// means that there is no longer any handle to the thread and no way to `join`
/// on it.
///
/// Due to platform restrictions, it is not possible to [`Clone`] this
/// handle: the ability to join a thread is a uniquely-owned permission.
///
/// This `struct` is created by the [`thread::spawn`] function and the
/// [`thread::Builder::spawn`] method.
///
/// # Examples
///
/// Creation from [`thread::spawn`]:
///
/// ```
/// use std::thread;
///
/// let join_handle: thread::JoinHandle<_> = thread::spawn(|| {
///     // some work here
/// });
/// ```
///
/// Creation from [`thread::Builder::spawn`]:
///
/// ```
/// use std::thread;
///
/// let builder = thread::Builder::new();
///
/// let join_handle: thread::JoinHandle<_> = builder.spawn(|| {
///     // some work here
/// }).unwrap();
/// ```
///
/// A thread being detached and outliving the thread that spawned it:
///
/// ```no_run
/// use std::thread;
/// use std::time::Duration;
///
/// let original_thread = thread::spawn(|| {
///     let _detached_thread = thread::spawn(|| {
///         // Here we sleep to make sure that the first thread returns before.
///         thread::sleep(Duration::from_millis(10));
///         // This will be called, even though the JoinHandle is dropped.
///         println!("♫ Still alive ♫");
///     });
/// });
///
/// original_thread.join().expect("The thread being joined has panicked");
/// println!("Original thread is joined.");
///
/// // We make sure that the new thread has time to run, before the main
/// // thread returns.
///
/// thread::sleep(Duration::from_millis(1000));
/// ```
///
/// [`thread::Builder::spawn`]: Builder::spawn
/// [`thread::spawn`]: spawn
#[stable(feature = "rust1", since = "1.0.0")]
pub struct JoinHandle<T>(JoinInner<'static, T>);

#[stable(feature = "joinhandle_impl_send_sync", since = "1.29.0")]
unsafe impl<T> Send for JoinHandle<T> {}
#[stable(feature = "joinhandle_impl_send_sync", since = "1.29.0")]
unsafe impl<T> Sync for JoinHandle<T> {}

impl<T> JoinHandle<T> {
    /// Extracts a handle to the underlying thread.
    ///
    /// # Examples
    ///
    /// ```
    /// use std::thread;
    ///
    /// let builder = thread::Builder::new();
    ///
    /// let join_handle: thread::JoinHandle<_> = builder.spawn(|| {
    ///     // some work here
    /// }).unwrap();
    ///
    /// let thread = join_handle.thread();
    /// println!("thread id: {:?}", thread.id());
    /// ```
    #[stable(feature = "rust1", since = "1.0.0")]
    #[must_use]
    pub fn thread(&self) -> &Thread {
        &self.0.thread
    }

    /// Waits for the associated thread to finish.
    ///
    /// This function will return immediately if the associated thread has already finished.
    ///
    /// In terms of [atomic memory orderings],  the completion of the associated
    /// thread synchronizes with this function returning. In other words, all
    /// operations performed by that thread [happen
    /// before](https://doc.rust-lang.org/nomicon/atomics.html#data-accesses) all
    /// operations that happen after `join` returns.
    ///
    /// If the associated thread panics, [`Err`] is returned with the parameter given
    /// to [`panic!`].
    ///
    /// [`Err`]: crate::result::Result::Err
    /// [atomic memory orderings]: crate::sync::atomic
    ///
    /// # Panics
    ///
    /// This function may panic on some platforms if a thread attempts to join
    /// itself or otherwise may create a deadlock with joining threads.
    ///
    /// # Examples
    ///
    /// ```
    /// use std::thread;
    ///
    /// let builder = thread::Builder::new();
    ///
    /// let join_handle: thread::JoinHandle<_> = builder.spawn(|| {
    ///     // some work here
    /// }).unwrap();
    /// join_handle.join().expect("Couldn't join on the associated thread");
    /// ```
    #[stable(feature = "rust1", since = "1.0.0")]
    pub fn join(self) -> Result<T> {
        self.0.join()
    }

    /// Checks if the associated thread has finished running its main function.
    ///
    /// `is_finished` supports implementing a non-blocking join operation, by checking
    /// `is_finished`, and calling `join` if it returns `true`. This function does not block. To
    /// block while waiting on the thread to finish, use [`join`][Self::join].
    ///
    /// This might return `true` for a brief moment after the thread's main
    /// function has returned, but before the thread itself has stopped running.
    /// However, once this returns `true`, [`join`][Self::join] can be expected
    /// to return quickly, without blocking for any significant amount of time.
    #[stable(feature = "thread_is_running", since = "1.61.0")]
    pub fn is_finished(&self) -> bool {
        Arc::strong_count(&self.0.packet) == 1
    }
}

impl<T> AsInner<imp::Thread> for JoinHandle<T> {
    fn as_inner(&self) -> &imp::Thread {
        &self.0.native
    }
}

impl<T> IntoInner<imp::Thread> for JoinHandle<T> {
    fn into_inner(self) -> imp::Thread {
        self.0.native
    }
}

#[stable(feature = "std_debug", since = "1.16.0")]
impl<T> fmt::Debug for JoinHandle<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("JoinHandle").finish_non_exhaustive()
    }
}

fn _assert_sync_and_send() {
    fn _assert_both<T: Send + Sync>() {}
    _assert_both::<JoinHandle<()>>();
    _assert_both::<Thread>();
}

/// Returns an estimate of the default amount of parallelism a program should use.
///
/// Parallelism is a resource. A given machine provides a certain capacity for
/// parallelism, i.e., a bound on the number of computations it can perform
/// simultaneously. This number often corresponds to the amount of CPUs a
/// computer has, but it may diverge in various cases.
///
/// Host environments such as VMs or container orchestrators may want to
/// restrict the amount of parallelism made available to programs in them. This
/// is often done to limit the potential impact of (unintentionally)
/// resource-intensive programs on other programs running on the same machine.
///
/// # Limitations
///
/// The purpose of this API is to provide an easy and portable way to query
/// the default amount of parallelism the program should use. Among other things it
/// does not expose information on NUMA regions, does not account for
/// differences in (co)processor capabilities or current system load,
/// and will not modify the program's global state in order to more accurately
/// query the amount of available parallelism.
///
/// Where both fixed steady-state and burst limits are available the steady-state
/// capacity will be used to ensure more predictable latencies.
///
/// Resource limits can be changed during the runtime of a program, therefore the value is
/// not cached and instead recomputed every time this function is called. It should not be
/// called from hot code.
///
/// The value returned by this function should be considered a simplified
/// approximation of the actual amount of parallelism available at any given
/// time. To get a more detailed or precise overview of the amount of
/// parallelism available to the program, you may wish to use
/// platform-specific APIs as well. The following platform limitations currently
/// apply to `available_parallelism`:
///
/// On Windows:
/// - It may undercount the amount of parallelism available on systems with more
///   than 64 logical CPUs. However, programs typically need specific support to
///   take advantage of more than 64 logical CPUs, and in the absence of such
///   support, the number returned by this function accurately reflects the
///   number of logical CPUs the program can use by default.
/// - It may overcount the amount of parallelism available on systems limited by
///   process-wide affinity masks, or job object limitations.
///
/// On Linux:
/// - It may overcount the amount of parallelism available when limited by a
///   process-wide affinity mask or cgroup quotas and `sched_getaffinity()` or cgroup fs can't be
///   queried, e.g. due to sandboxing.
/// - It may undercount the amount of parallelism if the current thread's affinity mask
///   does not reflect the process' cpuset, e.g. due to pinned threads.
/// - If the process is in a cgroup v1 cpu controller, this may need to
///   scan mountpoints to find the corresponding cgroup v1 controller,
///   which may take time on systems with large numbers of mountpoints.
///   (This does not apply to cgroup v2, or to processes not in a
///   cgroup.)
///
/// On all targets:
/// - It may overcount the amount of parallelism available when running in a VM
/// with CPU usage limits (e.g. an overcommitted host).
///
/// # Errors
///
/// This function will, but is not limited to, return errors in the following
/// cases:
///
/// - If the amount of parallelism is not known for the target platform.
/// - If the program lacks permission to query the amount of parallelism made
///   available to it.
///
/// # Examples
///
/// ```
/// # #![allow(dead_code)]
/// use std::{io, thread};
///
/// fn main() -> io::Result<()> {
///     let count = thread::available_parallelism()?.get();
///     assert!(count >= 1_usize);
///     Ok(())
/// }
/// ```
#[doc(alias = "available_concurrency")] // Alias for a previous name we gave this API on unstable.
#[doc(alias = "hardware_concurrency")] // Alias for C++ `std::thread::hardware_concurrency`.
#[doc(alias = "num_cpus")] // Alias for a popular ecosystem crate which provides similar functionality.
#[stable(feature = "available_parallelism", since = "1.59.0")]
pub fn available_parallelism() -> io::Result<NonZeroUsize> {
    imp::available_parallelism()
}
