use crate::print;
use core::{
    pin::Pin,
    task::{Context, Poll},
};
use crossbeam_queue::ArrayQueue;
use futures_util::stream::{Stream, StreamExt};
use futures_util::task::AtomicWaker;
use pc_keyboard::{layouts, DecodedKey, HandleControl, Keyboard, ScancodeSet1};
use spin::Once;

const SC_CAP: usize = 100;
/// Holds scancodes added by `add_scancode`.
static SCANCODE_QUEUE: Once<ArrayQueue<u8>> = Once::new();
static WAKER: AtomicWaker = AtomicWaker::new();

fn clear_array_queue<T>(queue: &ArrayQueue<T>) {
    while let Ok(_) = queue.pop() {}
}

/// Handles scancodes asynchronously.
pub async fn handle_scancodes() {
    let mut scancodes = ScancodeStream::new();
    let mut keyboard = Keyboard::new(layouts::Us104Key, ScancodeSet1, HandleControl::Ignore);

    while let Some(scancode) = scancodes.next().await {
        if let Ok(Some(key_event)) = keyboard.add_byte(scancode) {
            if let Some(key) = keyboard.process_keyevent(key_event) {
                match key {
                    DecodedKey::Unicode(character) => print!("{}", character),
                    DecodedKey::RawKey(_raw) => (),
                }
            }
        }
    }
}

/// Called by the keyboard interrupt handler.
///
/// Must not block or allocate.
pub(crate) fn add_scancode(scancode: u8) {
    use log::warn;

    if let Some(queue) = SCANCODE_QUEUE.r#try() {
        if let Err(_) = queue.push(scancode) {
            warn!("scancode queue full, clearing queue to avoid dropping keyboard input");
            clear_array_queue(&queue);
        } else {
            WAKER.wake();
        }
    } else {
        warn!("scancode queue uninitialized");
    }
}

/// Polls scancodes from `SCANCODE_QUEUE`.
pub struct ScancodeStream {
    _private: (),
}

impl ScancodeStream {
    pub fn new() -> Self {
        SCANCODE_QUEUE.call_once(|| ArrayQueue::new(SC_CAP));
        ScancodeStream { _private: () }
    }
}

impl Stream for ScancodeStream {
    type Item = u8;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context) -> Poll<Option<u8>> {
        let queue = SCANCODE_QUEUE.r#try().expect("not initialized");

        if let Ok(scancode) = queue.pop() {
            return Poll::Ready(Some(scancode));
        }

        WAKER.register(&cx.waker());
        match queue.pop() {
            Ok(scancode) => {
                WAKER.take();
                Poll::Ready(Some(scancode))
            }
            Err(crossbeam_queue::PopError) => Poll::Pending,
        }
    }
}
