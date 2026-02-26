//! Shared stream that converts QueryResult channel into Event stream.

use crate::db::QueryResult;
use crate::event::Event;
use futures::Stream;
use std::pin::Pin;
use std::task::{Context, Poll};
use tokio::sync::mpsc;

/// Stream that receives QueryResult and yields Event until EOSE.
pub struct QueryEventStream {
    pub(super) rx: mpsc::Receiver<QueryResult>,
}

impl Stream for QueryEventStream {
    type Item = Event;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Event>> {
        loop {
            match self.rx.poll_recv(cx) {
                Poll::Ready(Some(qr)) => {
                    if qr.event == "EOSE" {
                        return Poll::Ready(None);
                    }
                    if let Ok(event) = serde_json::from_str::<Event>(&qr.event) {
                        return Poll::Ready(Some(event));
                    }
                }
                Poll::Ready(None) => return Poll::Ready(None),
                Poll::Pending => return Poll::Pending,
            }
        }
    }
}
