//! The next frame the webview of a window presents. A page that settled its content has it in its DOM, not yet on the
//! screen: its webview paints it in a rendering update and commits the layers to the app's process, and a window shown
//! before that commit shows the layers of the last one (the shell, or the bare canvas) on its first frame. A window built
//! hidden shows after this frame (`show_window` in `commands/window_management.rs`).

use std::time::Duration;
use tauri::{Runtime, WebviewWindow};
use tokio::sync::oneshot;

/// What the wait for the next presentation of a page found
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Presentation {
  /// The webview committed a frame of the page as it was when asked: a window shown now shows it on its first frame
  Presented,
  /// No frame came within the budget: the page is stuck or the webview renders nothing while the window is hidden
  TimedOut,
  /// The webview tells no presentation (another platform, a `WebKit` without the call): the window shows at once
  Unknown,
}

/// Waits at most `budget` for the next frame that the webview of `window` presents of its page as it is now
pub async fn next_presentation<R: Runtime>(window: &WebviewWindow<R>, budget: Duration) -> Presentation {
  presented_within(budget, request_presentation(window)).await
}

/// The answer of a request of the next presentation (`presented`: true once the frame is presented, false or dropped
/// when the webview tells none), at most `budget` from now
pub async fn presented_within(budget: Duration, presented: oneshot::Receiver<bool>) -> Presentation {
  match tokio::time::timeout(budget, presented).await {
    Ok(Ok(true)) => Presentation::Presented,
    // no such call, or the webview went with its window
    Ok(Ok(false) | Err(_)) => Presentation::Unknown,
    Err(_) => Presentation::TimedOut,
  }
}

/// On macOS `-[WKWebView _doAfterNextPresentationUpdate:]` (`WKWebViewPrivate.h`, since macOS 10.12, which Safari's
/// tests rely on): the web process runs a rendering update for it, also while the window is hidden (the display link of
/// the page runs, `RemoteLayerTreeDrawingAreaProxyMac`), and `WebKit` calls the block once the app's process committed
/// the layers of that update. No public API says this; a `WKWebView` without the call (`respondsToSelector:`) answers
/// that it tells none, and the window shows at once, as before.
#[cfg(target_os = "macos")]
fn request_presentation<R: Runtime>(window: &WebviewWindow<R>) -> oneshot::Receiver<bool> {
  use tauri::Manager;

  let (sender, receiver) = oneshot::channel();
  let app = window.app_handle().clone();
  if let Err(e) = window.with_webview(move |webview| ask_webview(&app, webview.inner(), sender)) {
    tracing::warn!(window = window.label(), error = %e, "cannot ask the webview for its next frame");
  }
  receiver
}

/// Elsewhere the webview tells no presentation: the window shows at once
#[cfg(not(target_os = "macos"))]
fn request_presentation<R: Runtime>(_window: &WebviewWindow<R>) -> oneshot::Receiver<bool> {
  let (sender, receiver) = oneshot::channel();
  let _ = sender.send(false);
  receiver
}

/// Asks the `WKWebView` `view` for the block after its next presentation update, on the main thread (`with_webview`):
/// `presented` gets true once the layers of that update reached the render server, false without the call
#[cfg(target_os = "macos")]
#[expect(unsafe_code, reason = "a message to the WKWebView of wry, whose call no binding has")]
fn ask_webview<R: Runtime>(app: &tauri::AppHandle<R>, view: *mut std::ffi::c_void, presented: oneshot::Sender<bool>) {
  use objc2::runtime::AnyObject;
  use objc2::{msg_send, sel};

  // SAFETY: wry's `WKWebView`, alive while its window is: `with_webview` runs this on the main thread of the window
  let Some(view) = (unsafe { view.cast::<AnyObject>().as_ref() }) else {
    let _ = presented.send(false);
    return;
  };
  // SAFETY: `respondsToSelector:` of `NSObject`, which every `WKWebView` is
  let responds: bool = unsafe { msg_send![view, respondsToSelector: sel!(_doAfterNextPresentationUpdate:)] };
  if !responds {
    let _ = presented.send(false);
    return;
  }
  let presented = std::sync::Mutex::new(Some(presented));
  let app = app.clone();
  let block = block2::RcBlock::new(move || {
    let Some(presented) = presented.lock().unwrap_or_else(std::sync::PoisonError::into_inner).take() else {
      return;
    };
    // `WebKit` calls the block while it commits the layers, in the implicit transaction of Core Animation, which the main
    // thread flushes when it waits next: a busy main thread may not wait before it orders the window in, so a later turn
    // flushes the transaction to the render server, then the window may show
    let flushed = app.run_on_main_thread(move || {
      flush_transaction();
      let _ = presented.send(true);
    });
    if let Err(e) = flushed {
      tracing::warn!(error = %e, "cannot flush the frame the webview presented");
    }
  });
  // SAFETY: `-[WKWebView _doAfterNextPresentationUpdate:(void (^)(void))]`, which `respondsToSelector:` confirmed; the
  // webview copies the block
  let () = unsafe { msg_send![view, _doAfterNextPresentationUpdate: &*block] };
}

/// `[CATransaction flush]`: the layers changed in this turn of the main thread reach the render server now
#[cfg(target_os = "macos")]
#[expect(unsafe_code, reason = "a class message to CATransaction, which no binding of the app has")]
fn flush_transaction() {
  use objc2::msg_send;
  use objc2::runtime::AnyClass;

  if let Some(transaction) = AnyClass::get(c"CATransaction") {
    // SAFETY: `+[CATransaction flush]` of `QuartzCore`, which `AppKit` loaded, on the main thread (`run_on_main_thread`)
    let () = unsafe { msg_send![transaction, flush] };
  }
}
