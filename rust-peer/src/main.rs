
use anyhow::{Context, Result};
use clap::Parser;
use futures::future::{select, Either};
use futures::StreamExt;
use libp2p::{
    core::muxing::StreamMuxerBox,
    ping,
    dcutr,
    gossipsub, identify, identity,
    kad::store::MemoryStore,
    kad,
    mdns,
    memory_connection_limits,
    multiaddr::{Multiaddr, Protocol},
    relay, tcp,
    yamux, noise,
    swarm::{NetworkBehaviour, Swarm, SwarmEvent},
    SwarmBuilder,
    PeerId, StreamProtocol, Transport,
};
use libp2p_webrtc as webrtc;
use libp2p_webrtc::tokio::Certificate;
use log::{debug, error, info, warn};
use prost::Message;
use std::net::IpAddr;
use std::path::Path;
use std::{
    collections::hash_map::DefaultHasher,
    hash::{Hash, Hasher},
    time::Duration,
};
use tokio::fs;
use rand::Rng;

include!(concat!(env!("OUT_DIR"), "/peer.rs"));

const TICK_INTERVAL: Duration = Duration::from_secs(15);
const KADEMLIA_PROTOCOL_NAME: StreamProtocol =
    StreamProtocol::new("/universal-connectivity/lan/kad/1.0.0");
const PORT_WEBRTC: u16 = 9090;
const PORT_QUIC: u16 = 9091;
const PORT_TCP: u16 = 9092;
const LOCAL_KEY_PATH: &str = "./local_key";
const LOCAL_CERT_PATH: &str = "./cert.pem";
const GOSSIPSUB_PEER_DISCOVERY: &str = "constellation._peer-discovery._p2p._pubsub";
const BOOTSTRAP_NODES: [&str; 4] = [
    "/dnsaddr/bootstrap.libp2p.io/p2p/QmNnooDu7bfjPFoTZYxMNLWUQJyrVwtbZg5gBMjTezGAJN",
    "/dnsaddr/bootstrap.libp2p.io/p2p/QmQCU2EcMqAqQPR2i9bChDtGNJchTbq5TbXJJ16u19uLTa",
    "/dnsaddr/bootstrap.libp2p.io/p2p/QmbLHAnMoJPWSCR5Zhtx6BHJX9KiKNN6tpvbUcqanj75Nb",
    "/dnsaddr/bootstrap.libp2p.io/p2p/QmcZf59bWwK5XFi76CZX8cbJ4BhTzzA3gU1ZjYZcYW3dwt",
];



#[derive(Debug, Parser)]
#[clap(name = "universal connectivity rust peer")]

struct Opt {
    /// Address to listen on.
    #[clap(long, default_value = "0.0.0.0")]
    listen_address: IpAddr,

    /// If known, the external address of this node. Will be used to correctly advertise our external address across all transports.
    #[clap(long, env)]
    external_address: Option<IpAddr>,

    /// Nodes to connect to on startup. Can be specified several times.
    connect: Vec<Multiaddr>,

    /// Gossipsub peer discovery topic.
    #[clap(long, default_value = "constellation._peer-discovery._p2p._pubsub")]
    gossipsub_peer_discovery: String,
}

/// An example WebRTC peer that will accept connections
#[tokio::main]
async fn main() -> Result<()> {
    let mut rng = rand::thread_rng();
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();

    let opt = Opt::parse();
    let local_key = read_or_create_identity(Path::new(LOCAL_KEY_PATH))
        .await
        .context("Failed to read identity")?;
    let webrtc_cert = read_or_create_certificate(Path::new(LOCAL_CERT_PATH))
        .await
        .context("Failed to read certificate")?;

    let mut swarm = create_swarm(local_key.clone(), webrtc_cert, &opt)?;

    let address_webrtc = Multiaddr::from(opt.listen_address)
        .with(Protocol::Udp(PORT_WEBRTC))
        .with(Protocol::WebRTCDirect);

    let address_quic = Multiaddr::from(opt.listen_address)
        .with(Protocol::Udp(PORT_QUIC))
        .with(Protocol::QuicV1);

    let address_tcp = Multiaddr::from(opt.listen_address)
        .with(Protocol::Tcp(PORT_TCP));

    swarm
        .listen_on(address_webrtc.clone())
        .expect("listen on webrtc");
    swarm
        .listen_on(address_quic.clone())
        .expect("listen on quic");
    swarm
        .listen_on(address_tcp.clone())
        .expect("listen on tcp");

    for addr in opt.connect {
        if let Err(e) = swarm.dial(addr.clone()) {
            debug!("Failed to dial {addr}: {e}");
        }
    }

    for peer in &BOOTSTRAP_NODES {
        let multiaddr: Multiaddr = peer.parse().expect("Failed to parse Multiaddr");
        if let Err(e) = swarm.dial(multiaddr) {
            debug!("Failed to dial {peer}: {e}");
        }
    }

    let peer_discovery = gossipsub::IdentTopic::new(GOSSIPSUB_PEER_DISCOVERY).hash();

    let mut tick = futures_timer::Delay::new(TICK_INTERVAL);

    loop {
        match select(swarm.next(), &mut tick).await {
            Either::Left((event, _)) => match event.unwrap() {
                SwarmEvent::NewListenAddr { address, .. } => {
                    if let Some(external_ip) = opt.external_address {
                        let external_address = address
                            .replace(0, |_| Some(external_ip.into()))
                            .expect("address.len > 1 and we always return `Some`");

                        swarm.add_external_address(external_address);
                    }

                    let p2p_address = address.with(Protocol::P2p(*swarm.local_peer_id()));
                    info!("Listening on {p2p_address}");
                }
                SwarmEvent::ConnectionEstablished { peer_id, .. } => {
                    info!("Connected to {peer_id}");
                }
                SwarmEvent::OutgoingConnectionError { peer_id, error, .. } => {
                    warn!("Failed to dial {peer_id:?}: {error}");
                }
                SwarmEvent::IncomingConnectionError { error, .. } => {
                    warn!("{:#}", anyhow::Error::from(error))
                }
                SwarmEvent::ConnectionClosed { peer_id, cause, .. } => {
                    warn!("Connection to {peer_id} closed: {cause:?}");
                    swarm.behaviour_mut().kademlia.remove_peer(&peer_id);
                    info!("Removed {peer_id} from the routing table (if it was in there).");
                }
                SwarmEvent::Behaviour(BehaviourEvent::Relay(e)) => {
                    debug!("{:?}", e);
                }
                SwarmEvent::Behaviour(BehaviourEvent::Dcutr(e)) => {
                    info!("Connected to (through DCUTR) {:?}", e);
                }
                SwarmEvent::Behaviour(BehaviourEvent::Gossipsub(
                    libp2p::gossipsub::Event::Message {
                        message_id: _,
                        propagation_source: _,
                        message,
                    },
                )) => {

                    if message.topic == peer_discovery {
                        let peer = Peer::decode(&*message.data).unwrap();
                        // info!("Received peer from {:?}", peer.addrs);
                        let rand_num = rng.gen::<i32>();
                        let peer = Peer {
                            public_key: peer.public_key,
                            addrs: peer.addrs,
                            rand: Some(rand_num),
                        };
                        let mut buf = Vec::new();
                                peer.encode(&mut buf)?;

                        if let Err(err) = swarm.behaviour_mut().gossipsub.publish(
                                                    gossipsub::IdentTopic::new(GOSSIPSUB_PEER_DISCOVERY),
                                                    &*buf,)
                        {error!("190 Failed to publish peer: {err}")}

                        for addr in &peer.addrs {
                            if let Ok(multiaddr) = Multiaddr::try_from(addr.clone()) {
                                info!("Received address: {:?}", multiaddr.to_string());
                            } else {
                                error!("Failed to parse multiaddress");
                            }
                        }

                        continue;
                    }

                    error!("Unexpected gossipsub topic hash: {:?}", message.topic);
                }
                SwarmEvent::Behaviour(BehaviourEvent::Gossipsub(
                    libp2p::gossipsub::Event::Subscribed { peer_id, topic },
                )) => {
                    debug!("{peer_id} subscribed to {topic}");
                }
                SwarmEvent::Behaviour(BehaviourEvent::Identify(e)) => {
                    info!("BehaviourEvent::Identify {:?}", e);

                    if let identify::Event::Error { peer_id, error, connection_id: _ } = e {
                        match error {
                            libp2p::swarm::StreamUpgradeError::Timeout => {
                                // When a browser tab closes, we don't get a swarm event
                                // maybe there's a way to get this with TransportEvent
                                // but for now remove the peer from routing table if there's an Identify timeout
                                swarm.behaviour_mut().kademlia.remove_peer(&peer_id);
                                info!("Removed {peer_id} from the routing table (if it was in there).");
                            }
                            _ => {
                                debug!("{error}");
                            }
                        }
                    } else if let identify::Event::Received {
                        peer_id,
                        connection_id: _,
                        info:
                            identify::Info {
                                listen_addrs,
                                protocols,
                                observed_addr,
                                ..
                            },
                    } = e
                    {
                        debug!("identify::Event::Received observed_addr: {}", observed_addr);

                        swarm.add_external_address(observed_addr);

                        // TODO: The following should no longer be necessary after https://github.com/libp2p/rust-libp2p/pull/4371.
                        if protocols.iter().any(|p| p == &KADEMLIA_PROTOCOL_NAME) {
                            for addr in listen_addrs {
                                debug!("identify::Event::Received listen addr: {}", addr);
                                // TODO (fixme): the below doesn't work because the address is still missing /webrtc/p2p even after https://github.com/libp2p/js-libp2p-webrtc/pull/121
                                // swarm.behaviour_mut().kademlia.add_address(&peer_id, addr);

                                let webrtc_address = addr
                                    .with(Protocol::WebRTCDirect)
                                    .with(Protocol::P2p(peer_id));

                                swarm
                                    .behaviour_mut()
                                    .kademlia
                                    .add_address(&peer_id, webrtc_address.clone());
                                info!("Added {webrtc_address} to the routing table.");
                            }
                        }
                    }
                }
                SwarmEvent::Behaviour(BehaviourEvent::Kademlia(e)) => {
                    debug!("Kademlia event: {:?}", e);
                }
                event => {
                    debug!("Other type of event: {:?}", event);
                }
            },
            Either::Right(_) => {
                tick = futures_timer::Delay::new(TICK_INTERVAL);

                debug!(
                    "external addrs: {:?}",
                    swarm.external_addresses().collect::<Vec<&Multiaddr>>()
                );

                if let Err(e) = swarm.behaviour_mut().kademlia.bootstrap() {
                    debug!("Failed to run Kademlia bootstrap: {e:?}");
                }

                let peer = Peer {
                    public_key: local_key.clone().public().encode_protobuf(),
                    addrs: swarm.external_addresses().map(|a| a.to_vec()).collect(),
                    rand: Some(rng.gen::<i32>()),
                };
                let mut buf = Vec::new();
                peer.encode(&mut buf)?;
                if let Err(err) = swarm.behaviour_mut().gossipsub.publish(
                    gossipsub::IdentTopic::new(GOSSIPSUB_PEER_DISCOVERY),
                    &*buf,
                ) {
                    error!("287 Failed to publish peer: {err}")
                }
            }
        }
    }
}

#[derive(NetworkBehaviour)]
struct Behaviour {
//     relay_client: relay::client::Behaviour,
    ping: ping::Behaviour,
    dcutr: dcutr::Behaviour,
    gossipsub: gossipsub::Behaviour,
    identify: identify::Behaviour,
    kademlia: kad::Behaviour<MemoryStore>,
    relay: relay::Behaviour,
    mdns: mdns::tokio::Behaviour,
    connection_limits: memory_connection_limits::Behaviour,
}

fn create_swarm(
    local_key: identity::Keypair,
    certificate: Certificate,
    opt:&Opt
) -> Result<Swarm<Behaviour>> {
    let local_peer_id = PeerId::from(local_key.public());
    debug!("Local peer id: {local_peer_id}");

    // To content-address message, we can take the hash of message and use it as an ID.
    let message_id_fn = |message: &gossipsub::Message| {
        let mut s = DefaultHasher::new();
        message.data.hash(&mut s);
        gossipsub::MessageId::from(s.finish().to_string())
    };

    // Set a custom gossipsub configuration
    let gossipsub_config = gossipsub::ConfigBuilder::default()
        .validation_mode(gossipsub::ValidationMode::Permissive) // This sets the kind of message validation. The default is Strict (enforce message signing)
        .message_id_fn(message_id_fn) // content-address messages. No two messages of the same content will be propagated.
        .mesh_outbound_min(1)
        .mesh_n_low(1)
        .flood_publish(true)
        .build()
        .expect("Valid config");

    // build a gossipsub network behaviour
    let mut gossipsub = gossipsub::Behaviour::new(
        gossipsub::MessageAuthenticity::Signed(local_key.clone()),
        gossipsub_config,
    )
    .expect("Correct configuration");

    // Create/subscribe Gossipsub topics
    gossipsub.subscribe(&gossipsub::IdentTopic::new(&opt.gossipsub_peer_discovery))?;

    let identify_config = identify::Behaviour::new(
        identify::Config::new("/ipfs/0.1.0".into(), local_key.public())
            .with_interval(Duration::from_secs(60)), // do this so we can get timeouts for dropped WebRTC connections
    );

    // Create a Kademlia behaviour.
    let cfg = kad::Config::new(KADEMLIA_PROTOCOL_NAME);
    let store = MemoryStore::new(local_peer_id);
    let kad_behaviour = kad::Behaviour::with_config(local_peer_id, store, cfg);
    let behaviour = Behaviour {
        ping: ping::Behaviour::new(ping::Config::new()),
        dcutr: dcutr::Behaviour::new(local_key.public().to_peer_id()),
        gossipsub,
        identify: identify_config,
        kademlia: kad_behaviour,
        mdns: mdns::tokio::Behaviour::new(mdns::Config::default(), local_key.public().to_peer_id())?,
        relay: relay::Behaviour::new(
            local_peer_id,
            relay::Config {
                max_reservations: usize::MAX,
                max_reservations_per_peer: 100,
                reservation_rate_limiters: Vec::default(),
                circuit_src_rate_limiters: Vec::default(),
                max_circuits: usize::MAX,
                max_circuits_per_peer: 100,
                ..Default::default()
            },
        ),
        connection_limits: memory_connection_limits::Behaviour::with_max_percentage(0.9),
    };

    Ok(
        SwarmBuilder::with_existing_identity(local_key)
            .with_tokio()
            .with_tcp(
                tcp::Config::default().nodelay(true),
                noise::Config::new,
                yamux::Config::default,
            )?
            // let quic = quic::tokio::Transport::new(quic::Config::new(&local_key));
            .with_quic()
            .with_other_transport(|local_key| {
                Ok(webrtc::tokio::Transport::new(
                    local_key.clone(),
                    certificate,
                )
                .map(|(peer_id, conn), _| (peer_id, StreamMuxerBox::new(conn))))
            })?
            .with_dns()?
            .with_behaviour(|_| {
                Ok(behaviour)
            })?
            .with_swarm_config(|cfg| {
                cfg.with_idle_connection_timeout(Duration::from_secs(60))
            })
            .build(),
    )
}

async fn read_or_create_certificate(path: &Path) -> Result<Certificate> {
    if path.exists() {
        let pem = fs::read_to_string(&path).await?;

        info!("Using existing certificate from {}", path.display());

        return Ok(Certificate::from_pem(&pem)?);
    }

    let cert = Certificate::generate(&mut rand::thread_rng())?;
    fs::write(&path, &cert.serialize_pem().as_bytes()).await?;

    info!(
        "Generated new certificate and wrote it to {}",
        path.display()
    );

    Ok(cert)
}

async fn read_or_create_identity(path: &Path) -> Result<identity::Keypair> {
    if path.exists() {
        let bytes = fs::read(&path).await?;

        info!("Using existing identity from {}", path.display());

        return Ok(identity::Keypair::from_protobuf_encoding(&bytes)?); // This only works for ed25519 but that is what we are using.
    }

    let identity = identity::Keypair::generate_ed25519();

    fs::write(&path, &identity.to_protobuf_encoding()?).await?;

    info!("Generated new identity and wrote it to {}", path.display());

    Ok(identity)
}