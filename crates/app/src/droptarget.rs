//! Explicit links and files dropped on the main window.

use std::sync::mpsc::{Receiver, Sender, channel};
use std::sync::{Arc, atomic::AtomicBool, atomic::Ordering};
/// What was dropped: an address or a file path.
pub type Dropped = String;

/// Links and files explicitly dropped on the main window.
/// On Windows through the window's own OLE `IDropTarget` (browsers hand links over that way);
/// macOS and Linux have no such hook here: winit passes dropped files only, which the main
/// window reads from egui itself.
pub struct Drops {
    rx: Receiver<Dropped>,
    #[cfg_attr(not(windows), allow(dead_code))]
    tx: Sender<Dropped>,
    /// Something droppable is being dragged over the window (highlight).
    over: Arc<AtomicBool>,
    #[cfg(windows)]
    ole: Option<crate::win32::ole::Registration>,
}

impl Default for Drops {
    fn default() -> Self {
        let (tx, rx) = channel();
        Drops {
            rx,
            tx,
            over: Arc::new(AtomicBool::new(false)),
            #[cfg(windows)]
            ole: None,
        }
    }
}

impl Drops {
    /// What was dropped on the window titled `title` since the last call (it is registered
    /// as a drop target once it exists).
    #[cfg_attr(not(windows), allow(unused_variables))]
    pub fn poll(&mut self, ctx: &egui::Context, title: &str) -> Vec<Dropped> {
        #[cfg(windows)]
        if self.ole.is_none() {
            self.ole = crate::win32::ole::Registration::new(
                title,
                self.tx.clone(),
                self.over.clone(),
                ctx,
            );
        }
        self.rx.try_iter().collect()
    }

    /// Something droppable is over the window.
    pub fn over(&self) -> bool {
        self.over.load(Ordering::Relaxed)
    }
}
