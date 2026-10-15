# Sahne

Spotify'da (veya MPRIS destekleyen başka bir oynatıcıda) çalan şarkının sözlerini senkronize ve animasyonlu gösteren, ekranı albüm kapağının renklerine büründüren bir Linux masaüstü uygulaması.

> Geliştirme sürüyor. Şu an **1. aşama** tamam: oynatıcıya bağlanma, çalan şarkının bilgisi/kapağı ve canlı ilerleyen süre.

## Geliştirme ortamı

Arch / CachyOS:

```sh
sudo pacman -S --needed webkit2gtk-4.1 libayatana-appindicator base-devel playerctl
npm i -g pnpm
```

(Debian/Ubuntu ve Fedora komutları paketleme aşamasında eklenecek.)

```sh
pnpm install
pnpm tauri dev
```

## Testler

```sh
pnpm test                      # TypeScript (Vitest)
pnpm check                     # Svelte/TS tip kontrolü
cd src-tauri && cargo test     # Rust
```

## Spotify olmadan deneme: mock player

`dev-tools/mock-player`, D-Bus'ta `org.mpris.MediaPlayer2.sahnemock` adıyla sahte bir oynatıcı yayınlar.

```sh
cd dev-tools/mock-player
cargo run                      # normal
cargo run -- --zero-position   # Position hep 0 döndüren bozuk oynatıcıyı taklit eder
cargo run -- --track "Sanatçı|Başlık|Albüm|213.5|https://…/kapak.jpg"
```

Çalışırken terminale `p` (oynat/duraklat), `n` (sonraki), `b` (önceki), `s 60` (60. saniyeye git), `q` (çık) yazabilirsin.

Sahne'yi mock'a bağlamak için (Spotify açıkken bile):

```sh
SAHNE_PLAYER=sahnemock pnpm tauri dev
```

Ayrıntılı log için `RUST_LOG=sahne_lib=debug` ekle.

## MPRIS verisini elle kontrol etme

```sh
playerctl -l                        # bağlı oynatıcılar
playerctl -p spotify metadata       # başlık, sanatçı, kapak adresi, süre
playerctl -p spotify position       # saniye cinsinden konum
```

`position` çalarken hep `0` dönüyorsa Sahne bunu fark eder ve süreyi kendi saatiyle sayar; arayüzde bir uyarı görünür.

## Bilinen notlar

- NVIDIA sürücüsü + Wayland'de WebKitGTK "Error 71" ile çöker. Sahne NVIDIA sürücüsünü görünce `WEBKIT_DISABLE_DMABUF_RENDERER=1` ayarlar; bu davranışı ezmek için değişkeni kendin ayarlayabilirsin.
