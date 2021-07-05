use tokio::io::{self, AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use std::net::{SocketAddr};
use tokio::sync::mpsc;
use std::collections::HashMap;


#[derive(Debug)]
enum ClientCommand {
    SendText(String)
}


#[derive(Debug)]
enum ClientMessage {
    Join(SocketAddr, mpsc::Sender<ClientCommand>),
    Leave(SocketAddr),
    Say(SocketAddr, String),
    Command(SocketAddr, Vec<String>),
    ServerStat,
}

async fn manger(mut msg_rx: mpsc::Receiver<ClientMessage>) {
    use ClientMessage::*;

    let mut client_map = HashMap::new();
    let mut client_nickname: HashMap<SocketAddr, String> = HashMap::new();

    
    'msg_loop: while let Some(msg) = msg_rx.recv().await {
        match msg {
            Join(client, cmd_tx) => {
                client_map.insert(client, cmd_tx);
                println!("[manager] client {:?} joined", client);
            },
            Leave(client) => {
                client_map.remove(&client);
                println!("[manager] client {:?} leaved", client);
            },
            ServerStat => {
                println!("[manager] server stat: {:?}", client_map.keys())
            },
            Say(client, text) => {
                for (c, cmd_tx) in &client_map {
                    if *c == client {
                        continue
                    }

                    let nickname = client_nickname
                        .get(&client)
                        .map(|n| n.to_owned())
                        .unwrap_or_else(|| format!("{}", client));
                    
                    let text = format!("{}: {}", nickname, text);
                    cmd_tx.send(ClientCommand::SendText(text)).await.unwrap();
                }
            },
            Command(client, commands) => {
                let cmd_tx =  match client_map.get(&client) {
                    Some(tx) => tx,
                    None => continue 'msg_loop,
                };

                let command = &commands[0];
                let args = &commands[1..];

                match command.as_str() {
                    "/setnickname" if args.len() == 1 => {
                        let nickname = args[0].clone();
                        client_nickname.insert(client, nickname);
                        cmd_tx.send(ClientCommand::SendText("ok\n".to_string())).await.unwrap();
                    },
                    _ => {
                        let text = "invalid args or no match pattern\n".to_string();
                        cmd_tx.send(ClientCommand::SendText(text)).await.unwrap();
                    }
                }
            },
        }
    }
}

#[tokio::main]
async fn main() -> io::Result<()> {
    let addr = "127.0.0.1:6381";
    let listener = TcpListener::bind(addr).await?;
    println!("listening at {}", addr);

    // spawn manager
    let (msg_tx, msg_rx) = mpsc::channel::<ClientMessage>(32);
    tokio::spawn(manger(msg_rx));

    loop {
        match listener.accept().await {
            Ok((socket, addr)) => {
                let cmd_tx = msg_tx.clone();
                tokio::spawn(async move {
                    if let Err(err) = process_socket(socket, addr, cmd_tx).await {
                        println!("process socket({:?}) error: {:?}", addr, err);
                    }
                });
            },
            Err(err) => {
                println!("accept error {:?}", err)
            }
        }
    }
}

async fn process_socket(socket: TcpStream, addr: SocketAddr, msg_tx: mpsc::Sender<ClientMessage>) -> io::Result<()> {
    println!("accepted {}", addr);

    let (cmd_tx, mut cmd_rx) = mpsc::channel::<ClientCommand>(4);

    // tell manager client join
    msg_tx.send(ClientMessage::Join(addr.clone(), cmd_tx)).await.unwrap();


    let (mut rd, mut wr) = io::split(socket);

    msg_tx.send(ClientMessage::ServerStat).await.unwrap();

    // reader
    let reader_msg_tx = msg_tx.clone();
    let reader = tokio::spawn(async move {
        loop {
            let mut buf = [0; 1024];
            match rd.read(&mut buf).await {
                Ok(0) => {
                    // EOF
                    return
                },
                Ok(n) => {
                    // read buffer
                    let text: String = String::from_utf8_lossy(&buf[..n]).into();
                    if !text.starts_with("/") {
                        reader_msg_tx.send(ClientMessage::Say(addr.clone(), text)).await.unwrap();
                        continue
                    }

                    let commands = text
                        .split_ascii_whitespace()
                        .map(|x| x.to_string())
                        .collect::<Vec<String>>();
                    reader_msg_tx.send(ClientMessage::Command(addr.clone(), commands)).await.unwrap();
                    
                },
                Err(err) => {
                    println!("[client] {:?} read bytes error {:?}", addr.clone(), err);
                    return
                }
            }
        }
    });

    // writer
    let writer = tokio::spawn(async move {
        while let Some(cmd) = cmd_rx.recv().await {
            use ClientCommand::*;
            match cmd {
                SendText(text) => {
                    if let Err(err) = wr.write(text.as_bytes()).await {
                        println!("[client] {:?} write bytes error {:?}", addr.clone(), err);
                        return 
                    }
                }
            }
        }
    });

    reader.await.unwrap();
    writer.await.unwrap();

    msg_tx.send(ClientMessage::Leave(addr)).await.unwrap();

    Ok(())
}