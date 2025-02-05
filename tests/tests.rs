use bmrng::{unbounded::UnboundedRequestReceiverStream, error::*, RequestReceiverStream, Request, Payload, unbounded::UnboundedPayload};
use futures_util::stream::StreamExt;
use tokio::time::{advance, pause, resume, sleep, Duration};

#[derive(Debug, PartialEq)]
struct Req(u32);

impl Request for Req {
    type Response = u32;
}

#[tokio::test]
async fn unbounded_send_receive() {
    let (tx, mut rx) = bmrng::unbounded_channel::<Req>();
    tokio::spawn(async move {
        let UnboundedPayload { request, responder } = rx.recv().await.expect("Unexpected err");
        assert!(!responder.is_closed());
        let res = responder.respond(request.0 + 4);
        assert!(res.is_ok());
    });
    assert!(!tx.is_closed());
    let response = tx.send_receive(Req(8)).await;
    assert!(tx.is_closed());
    assert_eq!(response, Ok(12));
}

#[tokio::test]
async fn bounded_send_receive() {
    let (tx, mut rx) = bmrng::channel::<Req>(1);
    tokio::spawn(async move {
        let Payload { request, responder } = rx.recv().await.expect("Unexpected err");
        assert!(!responder.is_closed());
        let res = responder.respond(request.0 + 4);
        assert!(res.is_ok());
    });
    assert!(!tx.is_closed());
    let response = tx.send_receive(Req(8)).await;
    assert!(tx.is_closed());
    assert_eq!(response, Ok(12));
}

#[tokio::test]
async fn unbounded_request_sender_clone() {
    let (tx, mut rx) = bmrng::unbounded_channel::<Req>();
    let tx2 = tx.clone();
    tokio::spawn(async move {
        let response = tx2.send_receive(Req(7)).await;
        assert_eq!(response, Ok(49));
    });
    tokio::spawn(async move {
        while let Ok(UnboundedPayload { request, responder }) = rx.recv().await {
            assert!(!responder.is_closed());
            let res = responder.respond(request.0 + 4);
            assert!(res.is_ok());
        }
    });

    assert!(!tx.is_closed());
    let response = tx.send_receive(Req(8)).await;
    assert_eq!(response, Ok(12));
}

#[tokio::test]
async fn bounded_request_sender_clone() {
    let (tx, mut rx) = bmrng::channel::<Req>(1);
    let tx2 = tx.clone();
    tokio::spawn(async move {
        let response = tx2.send_receive(Req(7)).await;
        assert_eq!(response, Ok(11));
    });
    tokio::spawn(async move {
        while let Ok(Payload { request, responder }) = rx.recv().await {
            assert!(!responder.is_closed());
            let res = responder.respond(request.0 + 4);
            assert!(res.is_ok());
        }
    });

    assert!(!tx.is_closed());
    let response = tx.send_receive(Req(8)).await;
    assert_eq!(response, Ok(12));
}

#[tokio::test]
async fn unbounded_drop_while_waiting_for_response() {
    let (tx, mut rx) = bmrng::unbounded_channel::<Req>();
    let task = tokio::spawn(async move {
        let UnboundedPayload { request: _, responder } = rx.recv().await.expect("Unexpected err");
        drop(responder);
    });
    let response = tx.send_receive(Req(8)).await;
    assert!(tokio::join!(task).0.is_ok());
    assert_eq!(response, Err(RequestError::RecvError));
}

#[tokio::test]
async fn unbounded_drop_while_waiting_for_request() {
    let (tx, mut rx) = bmrng::unbounded_channel::<Req>();
    let task = tokio::spawn(async move {
        if rx.recv().await.is_ok() {
            panic!("this should not be ok")
        };
    });
    drop(tx);
    assert!(tokio::join!(task).0.is_ok());
}

#[tokio::test]
async fn unbounded_drop_sender_while_sending_response() {
    let (tx, mut rx) = bmrng::unbounded_channel::<Req>();
    let task = tokio::spawn(async move {
        let UnboundedPayload { request: _, responder } = rx.recv().await.expect("Received err");
        let respond_result = responder.respond(42);
        assert_eq!(respond_result, Err(RespondError(42)));
    });
    let response_receiver = tx.send(Req(21));
    drop(response_receiver);
    assert!(tokio::join!(task).0.is_ok());
}

#[tokio::test]
async fn bounded_drop_while_waiting_for_response() {
    let (tx, mut rx) = bmrng::channel::<Req>(1);
    let task = tokio::spawn(async move {
        let Payload { request: _, responder } = rx.recv().await.expect("Unexpected err");
        drop(responder);
    });
    let response = tx.send_receive(Req(8)).await;
    assert!(tokio::join!(task).0.is_ok());
    assert_eq!(response, Err(RequestError::RecvError));
}

#[tokio::test]
async fn bounded_drop_while_waiting_for_request() {
    let (tx, mut rx) = bmrng::channel::<Req>(1);
    let task = tokio::spawn(async move {
        rx.recv().await.expect_err("this should not be ok");
    });
    drop(tx);
    assert!(tokio::join!(task).0.is_ok());
}

#[tokio::test]
async fn bounded_drop_sender_while_sending_response() {
    let (tx, mut rx) = bmrng::channel::<Req>(1);
    let task = tokio::spawn(async move {
        let Payload { request: _, responder } = rx.recv().await.expect("Unexpected err");
        let respond_result = responder.respond(42);
        assert_eq!(respond_result, Err(RespondError(42)));
    });
    let response_receiver = tx.send(Req(21)).await;
    drop(response_receiver);
    assert!(tokio::join!(task).0.is_ok());
}

#[tokio::test]
async fn bounded_close_request_receiver() {
    let (tx, mut rx) = bmrng::channel::<Req>(4);
    let task = tokio::spawn(async move {
        rx.close();
        let Payload { request, responder } = rx.recv().await.unwrap();
        assert!(responder.respond(request.0 * 2).is_ok());
    });
    let mut response_receiver = tx.send(Req(21)).await.unwrap();
    let response = response_receiver.recv().await;
    assert_eq!(response, Ok(42));
    drop(response_receiver);
    assert!(tx.send(Req(1)).await.is_err());
    assert!(tokio::join!(task).0.is_ok());
}

#[tokio::test]
async fn unbounded_close_request_receiver() {
    let (tx, mut rx) = bmrng::unbounded_channel::<Req>();
    let task = tokio::spawn(async move {
        rx.close();
        let UnboundedPayload { request, responder } = rx.recv().await.unwrap();
        assert!(responder.respond(request.0 * 2).is_ok());
    });
    let mut response_receiver = tx.send(Req(21)).unwrap();
    let response = response_receiver.recv().await;
    assert_eq!(response, Ok(42));
    drop(response_receiver);
    assert!(tx.send(Req(1)).is_err());
    assert!(tokio::join!(task).0.is_ok());
}

#[tokio::test]
async fn bounded_timeout() {
    let (tx, mut rx) = bmrng::channel_with_timeout::<Req>(1, Duration::from_millis(100));
    pause();
    tokio::spawn(async move {
        let Payload { request: _, responder } = rx.recv().await.expect("Unexpected err");
        assert!(!responder.is_closed());
        advance(Duration::from_millis(200)).await;
        sleep(Duration::from_micros(1)).await;
        resume();
        panic!("Should have timed out");
    });
    assert!(!tx.is_closed());
    let response = tx.send_receive(Req(8)).await;
    assert_eq!(response, Err(RequestError::<Req>::RecvTimeoutError));
}

#[tokio::test]
async fn unbounded_timeout() {
    let (tx, mut rx) =
        bmrng::unbounded::channel_with_timeout::<Req>(Duration::from_millis(100));
    pause();
    tokio::spawn(async move {
        let UnboundedPayload { request: _, responder } = rx.recv().await.expect("Unexpected err");
        assert!(!responder.is_closed());
        advance(Duration::from_millis(200)).await;
        sleep(Duration::from_micros(1)).await;
        resume();
        panic!("Should have timed out");
    });
    assert!(!tx.is_closed());
    let response = tx.send_receive(Req(8)).await;
    assert_eq!(response, Err(RequestError::<Req>::RecvTimeoutError));
}

#[tokio::test]
async fn bounded_stream() {
    let (tx, rx) = bmrng::channel::<Req>(1);
    tokio::spawn(async move {
        let mut stream = rx.into_stream();
        while let Some(Payload { request, responder }) = stream.next().await {
            assert_eq!(responder.is_closed(), false);
            let res = responder.respond(request.0 + 4);
            assert!(res.is_ok());
        }
    });
    assert!(!tx.is_closed());
    assert_eq!(tx.send_receive(Req(8)).await, Ok(12));
    assert!(!tx.is_closed());
    assert_eq!(tx.send_receive(Req(3)).await, Ok(7));
    assert!(!tx.is_closed());
    assert_eq!(tx.send_receive(Req(1)).await, Ok(5));
    assert!(!tx.is_closed());
}

#[tokio::test]
async fn unbounded_stream() {
    let (tx, rx) = bmrng::unbounded_channel::<Req>();
    tokio::spawn(async move {
        let mut stream = rx.into_stream();
        while let Some(UnboundedPayload { request, responder }) = stream.next().await {
            assert!(!responder.is_closed());
            let res = responder.respond(request.0 + 4);
            assert!(res.is_ok());
        }
    });
    assert!(!tx.is_closed());
    assert_eq!(tx.send_receive(Req(8)).await, Ok(12));
    assert!(!tx.is_closed());
    assert_eq!(tx.send_receive(Req(3)).await, Ok(7));
    assert!(!tx.is_closed());
    assert_eq!(tx.send_receive(Req(1)).await, Ok(5));
    assert!(!tx.is_closed());
}

#[tokio::test]
async fn req_receiver_into_inner() {
    let (tx, rx) = bmrng::channel::<Req>(1);
    let stream = RequestReceiverStream::new(rx);
    let mut rx = stream.into_inner();
    tokio::spawn(async move {
        let Payload { request, responder } = rx.recv().await.expect("Unexpected err");
        assert!(!responder.is_closed());
        let res = responder.respond(request.0 * 4);
        assert!(res.is_ok());
    });
    assert!(!tx.is_closed());
    let response = tx.send_receive(Req(8)).await;
    assert!(tx.is_closed());
    assert_eq!(response, Ok(32));
}

#[tokio::test]
async fn req_unbounded_receiver_into_inner() {
    let (tx, rx) = bmrng::unbounded_channel::<Req>();
    let stream = UnboundedRequestReceiverStream::new(rx);
    let mut rx = stream.into_inner();
    tokio::spawn(async move {
        let UnboundedPayload { request, responder } = rx.recv().await.expect("Unexpected err");
        assert!(!responder.is_closed());
        let res = responder.respond(request.0 * 4);
        assert!(res.is_ok());
    });
    assert!(!tx.is_closed());
    let response = tx.send_receive(Req(8)).await;
    assert!(tx.is_closed());
    assert_eq!(response, Ok(32));
}
