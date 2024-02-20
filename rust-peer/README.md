# Gestion sur DigitalOcean

## Configuration de la goutelette


### Ajout d'espace de mémoire vive
https://www.digitalocean.com/community/tutorials/how-to-add-swap-space-on-ubuntu-20-04
(1,5 Go)

### Configuration
```sh
apt install -y protobuf-compiler
apt install build-essential

git clone https://github.com/reseau-constellation/connectivite-rouille.git
cd connectivite-rouille/rust-peer
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
source "$HOME/.cargo/env"
cargo run
```

### Mise à jour client
```sh
cd connectivit-rouille
git pull
```


### Mise à jour de Rouille
```sh
rustup update
```

### Désinstaller Rouille
```sh
rustup self uninstall
```

## Résultats
Local peer id: 12D3KooWJ7P1yeoxB5mq3TwQh8YgmVhankjtT4rsVGZPUyf617aR
Listening on /ip4/127.0.0.1/udp/9090/webrtc-direct/certhash/uEiAJOkKT64u6jmXV5YxncCoER5WXSO2HYE23Xpap651xMw/p2p/12D3KooWJ7P1yeoxB5mq3TwQh8YgmVhankjtT4rsVGZPUyf617aR
Listening on /ip4/164.90.222.145/udp/9090/webrtc-direct/certhash/uEiAJOkKT64u6jmXV5YxncCoER5WXSO2HYE23Xpap651xMw/p2p/12D3KooWJ7P1yeoxB5mq3TwQh8YgmVhankjtT4rsVGZPUyf617aR
Listening on /ip4/10.19.0.5/udp/9090/webrtc-direct/certhash/uEiAJOkKT64u6jmXV5YxncCoER5WXSO2HYE23Xpap651xMw/p2p/12D3KooWJ7P1yeoxB5mq3TwQh8YgmVhankjtT4rsVGZPUyf617aR
Listening on /ip4/10.114.0.2/udp/9090/webrtc-direct/certhash/uEiAJOkKT64u6jmXV5YxncCoER5WXSO2HYE23Xpap651xMw/p2p/12D3KooWJ7P1yeoxB5mq3TwQh8YgmVhankjtT4rsVGZPUyf617aR
Listening on /ip4/127.0.0.1/udp/9091/quic-v1/p2p/12D3KooWJ7P1yeoxB5mq3TwQh8YgmVhankjtT4rsVGZPUyf617aR
Listening on /ip4/164.90.222.145/udp/9091/quic-v1/p2p/12D3KooWJ7P1yeoxB5mq3TwQh8YgmVhankjtT4rsVGZPUyf617aR
Listening on /ip4/10.19.0.5/udp/9091/quic-v1/p2p/12D3KooWJ7P1yeoxB5mq3TwQh8YgmVhankjtT4rsVGZPUyf617aR
Listening on /ip4/10.114.0.2/udp/9091/quic-v1/p2p/12D3KooWJ7P1yeoxB5mq3TwQh8YgmVhankjtT4rsVGZPUyf617aR
Listening on /ip4/127.0.0.1/tcp/9092/p2p/12D3KooWJ7P1yeoxB5mq3TwQh8YgmVhankjtT4rsVGZPUyf617aR
Listening on /ip4/164.90.222.145/tcp/9092/p2p/12D3KooWJ7P1yeoxB5mq3TwQh8YgmVhankjtT4rsVGZPUyf617aR
Listening on /ip4/10.19.0.5/tcp/9092/p2p/12D3KooWJ7P1yeoxB5mq3TwQh8YgmVhankjtT4rsVGZPUyf617aR
Listening on /ip4/10.114.0.2/tcp/9092/p2p/12D3KooWJ7P1yeoxB5mq3TwQh8YgmVhankjtT4rsVGZPUyf617aR

## Supervisor

https://www.digitalocean.com/community/tutorials/how-to-install-and-manage-supervisor-on-ubuntu-and-debian-vps

sudo apt update && sudo apt install supervisor
sudo systemctl status supervisor
sudo nano ~/relai.sh

```sh
#!/bin/bash
cd /root/connectivite-rouille/rust-peer
/root/.cargo/bin/cargo run
```
chmod +x ~/relai.sh
sudo nano /etc/supervisor/conf.d/relai.conf

```sh
[program:relai]
command=/root/relai.sh
autostart=true
autorestart=true
stderr_logfile=/var/log/relai.err.log
stdout_logfile=/var/log/relai.out.log
```

sudo supervisorctl reread
sudo supervisorctl update

```sh
sudo supervisorctl
> tail relai stdout
> tail relai stderr
```