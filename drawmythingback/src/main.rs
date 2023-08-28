use actix::prelude::ContextFutureSpawner;
use actix::{ Actor, Addr, Context, Handler, Message, StreamHandler, AsyncContext, Running };
use actix_web::web::{ Data, Bytes };
use actix_web::{ web, App, Error, HttpRequest, HttpResponse, HttpServer };
use actix_web_actors::ws;
use actix::WrapFuture;
use actix::ActorFutureExt;
use uuid::Uuid;
/// Define HTTP actor
#[actix_web::main]
async fn main() -> std::io::Result<()> {
    std::env::set_var("RUST_LOG", "actix_web=debug");
    env_logger::init();
    let server = Data::new((Server { clients: Vec::new() }).start());

    HttpServer::new(move || {
        App::new().app_data(server.clone()).route("/ws/", web::get().to(ws_index))
    })
        .bind("127.0.0.1:8080")?
        .run().await
}
const FRAME_SIZE: usize = 9_765_625 * 4;
async fn ws_index(
    req: HttpRequest,
    stream: web::Payload,
    srv: web::Data<Addr<Server>>
) -> Result<HttpResponse, Error> {
    let ws = MyWs::new(srv.get_ref().clone());

    // let resp = ws::start(ws, &req, stream).map_err(|e| {
    //     eprintln!("WebSocket Error {:?}", e);
    //     e
    // });
    ws::WsResponseBuilder::new(ws, &req, stream).frame_size(FRAME_SIZE).start()
    // resp
}
impl Handler<ClientConnected> for Server {
    type Result = ();

    fn handle(&mut self, msg: ClientConnected, _: &mut Context<Self>) -> Self::Result {
        self.clients.push(Client { id: msg.id, addr: msg.addr });
    }
}

impl StreamHandler<Result<ws::Message, ws::ProtocolError>> for MyWs {
    fn handle(&mut self, msg: Result<ws::Message, ws::ProtocolError>, _ctx: &mut Self::Context) {
        match msg {
            Ok(ws::Message::Text(text)) => {
                self.server_addr.do_send(BroadcastText {
                    id: self.id,
                    text: ClientMessage::Text(text.to_string()),
                });
            }
            Ok(ws::Message::Binary(bin)) => {
                self.server_addr.do_send(BroadcastText {
                    id: self.id,
                    text: ClientMessage::Binary(bin),
                });
            }
            _ => {}
        }
    }
}
#[derive(Message)]
#[rtype(result = "()")]
struct ClientConnected {
    addr: Addr<MyWs>,
    id: Uuid,
}

#[derive(Message)]
#[rtype(result = "()")]
struct ClientText(ClientMessage);

impl Actor for MyWs {
    type Context = ws::WebsocketContext<Self>;

    fn started(&mut self, ctx: &mut Self::Context) {
        let addr = ctx.address();
        self.server_addr
            .send(ClientConnected { id: self.id, addr: addr.clone() })
            .into_actor(self)
            .map(|_, _, _| ())
            .wait(ctx);
        eprintln!("Client connected {}", self.id);
    }
    fn stopping(&mut self, _: &mut Self::Context) -> Running {
        // This will be logged when a client is about to disconnect
        println!("Client with ID: {} disconnecting", self.id);
        Running::Stop
    }
}

impl Handler<ClientText> for MyWs {
    type Result = ();

    fn handle(&mut self, msg: ClientText, ctx: &mut Self::Context) {
        eprintln!("Message sent form client with ID: {}", self.id);
        match msg.0 {
            ClientMessage::Text(text) => {
                ctx.text(text);
            }
            ClientMessage::Binary(bin) => {
                ctx.binary(bin);
            }
        }
    }
}
struct Server {
    clients: Vec<Client>,
}
struct Client {
    id: Uuid,
    addr: Addr<MyWs>,
}
impl Actor for Server {
    type Context = Context<Self>;
}

#[derive(Message)]
#[rtype(result = "()")]
struct BroadcastText {
    id: Uuid,
    text: ClientMessage,
}

#[derive(Message, Clone)]
#[rtype(result = "()")]
enum ClientMessage {
    Text(String),
    Binary(Bytes),
}
impl Handler<BroadcastText> for Server {
    type Result = ();

    fn handle(&mut self, msg: BroadcastText, _: &mut Context<Self>) -> Self::Result {
        for client in &self.clients {
            if client.id != msg.id {
                client.addr.do_send(ClientText(msg.text.clone()));
            }
            if client.id == msg.id {
                client.addr.do_send(ClientText(ClientMessage::Text(String::from("okay"))));
            }
        }
    }
}
struct MyWs {
    server_addr: Addr<Server>,
    id: Uuid,
}

impl MyWs {
    fn new(server_addr: Addr<Server>) -> Self {
        MyWs { server_addr, id: Uuid::new_v4() }
    }
}
