# Sahne

Spotify'da (veya MPRIS destekleyen başka bir oynatıcıda) çalan şarkının sözlerini **senkronize ve kelime kelime animasyonlu** olarak tam ekranda gösteren, ekranı albüm kapağının renklerine büründüren bir Linux masaüstü uygulaması. İsteğe bağlı olarak RGB klavye ve LED'leri de (OpenRGB) aynı renklere boyar.

- Spotify Web API, hesap, anahtar veya Premium **gerekmez**: oynatıcı bilgisi D-Bus/MPRIS'ten okunur.
- Sözler ücretsiz ve anahtarsız [LRCLIB](https://lrclib.net)'ten gelir, diskte önbelleğe alınır.
- Wayland ve X11'de çalışır; GNOME ve KDE hedeflenmiştir.

## Kurulum

`pnpm tauri build` iki paket üretir:

- `src-tauri/target/release/bundle/appimage/Sahne_<sürüm>_amd64.AppImage` — tek dosya, kurulum gerektirmez (`chmod +x` ile çalıştır).
- `src-tauri/target/release/bundle/deb/Sahne_<sürüm>_amd64.deb` — `sudo apt install ./Sahne_*.deb`.

## Kullanım

Spotify'ı açıp bir şarkı çal, Sahne'yi başlat. Birkaç yüz milisaniye içinde şarkıyı tanır, sözleri bulur ve sahneyi kurar.

| Tuş | İşlev |
|---|---|
| `Boşluk` | Oynat / duraklat |
| `←` / `→` | Önceki / sonraki şarkı |
| `F` / `Esc` | Tam ekran / tam ekrandan çık |
| `M` | Mini mod (küçük, her zaman üstte pencere) |
| `S` | Animasyon stilini değiştir: **Klasik → Kinetik → Sade** |
| `[` / `]` | Bu şarkının sözlerini 100 ms öne al / geciktir (kalıcı) |
| `,` | Ayarlar |

Söz satırına tıklamak şarkıyı o satıra sarar; alt çubuktaki ilerleme çubuğu da tıklanabilir. Fare 2 saniye hareketsiz kalınca kontroller gizlenir.

**Animasyon stilleri**

- **Klasik:** Aktif satır ortada ve büyük; kelimeler söylendikçe soldan sağa dolar, hafifçe zıplar ve parlar. Önceki/sonraki satırlar küçük ve soluk.
- **Kinetik:** Her kelime tek başına ekranın ortasında, ölçek ve hafif dönmeyle belirir.
- **Sade:** Hareket yok, yalnızca renk dolumu. Sistemde "hareketi azalt" (`prefers-reduced-motion`) açıksa otomatik seçilir.

LRCLIB'deki sözlerin çoğu satır düzeyinde zamanlıdır. Kelime zamanları (Enhanced LRC) yoksa satır süresi kelimelere uzunluklarına göre dağıtılır; bu yüzden kelime vurgusu bir tahmindir, satır geçişleri ise gerçek zamanlıdır.

## Ayarlar ve dosyalar

| Yol | İçerik |
|---|---|
| `~/.config/sahne/settings.json` | Ayarlar (panelden değişir; elle düzenlenebilir) |
| `~/.config/sahne/offsets.json` | Şarkı başına söz kaydırmaları |
| `~/.cache/sahne/lyrics/` | Söz önbelleği (bulunamayanlar 24 saat sonra yeniden aranır) |
| `~/.cache/sahne/art/` | Kapak ve önceden bulanıklaştırılmış arka plan önbelleği |

Önbelleği silmek güvenlidir.

## Geliştirme

### Sistem bağımlılıkları

**Debian / Ubuntu**

```sh
sudo apt update
sudo apt install libwebkit2gtk-4.1-dev build-essential curl wget file libxdo-dev \
  libssl-dev libayatana-appindicator3-dev librsvg2-dev playerctl
```

**Fedora**

```sh
sudo dnf check-update
sudo dnf install webkit2gtk4.1-devel openssl-devel curl wget file libappindicator-gtk3-devel \
  librsvg2-devel libxdo-devel playerctl
sudo dnf group install "c-development"
```

**Arch / CachyOS / Manjaro**

```sh
sudo pacman -S --needed webkit2gtk-4.1 base-devel curl wget file openssl appmenu-gtk-module \
  libappindicator-gtk3 librsvg xdotool playerctl
```

Ayrıca: Rust (stable, `rustup`), Node.js 20+ ve pnpm (`npm i -g pnpm`; nvm kullanıyorsan `sudo` olmadan).

### Çalıştırma ve testler

```sh
pnpm install
pnpm tauri dev                 # geliştirme
pnpm tauri build               # AppImage + .deb

pnpm test                      # TypeScript birim testleri (Vitest)
pnpm check                     # Svelte/TS tip kontrolü
cd src-tauri && cargo test     # Rust birim testleri
```

Ayrıntılı log için `RUST_LOG=sahne_lib=debug pnpm tauri dev`. Arayüzdeki JS hataları da `frontend` hedefiyle aynı loga düşer.

### Mimari

```
src-tauri/src/
  player/     MPRIS izleme, oynatıcı seçimi, konum takibi (Position=0 yedeği), kontroller
  lyrics/     LRCLIB istemcisi, LRC ayrıştırıcı, başlık temizleme, önbellek, offset'ler
  artwork/    kapak indirme/önbellek, OKLab k-means palet, bulanık arka plan
  lighting/   OpenRGB (opsiyonel), renk kareleri ve geçişler
  settings.rs, windows.rs (mini mod), events.rs (frontend'e giden olay tipleri)
src/
  lib/stores/       player, lyrics, palette, settings, lighting, ui
  lib/components/   LyricsView (klasik/sade), KineticView, Background, ControlBar, SettingsPanel…
  lib/timing.ts     aktif satır/kelime, kelime zamanlaması, enstrümantal aralar
  lib/clock.ts      backend senkronları arasında 60 fps konum interpolasyonu
  MiniApp.svelte    mini pencere
```

Backend → frontend olayları: `track-changed`, `playback-state`, `position-sync`, `lyrics-loaded`, `palette-changed`, `player-disconnected`, `settings-changed`, `lighting-status`. Tipler `src-tauri/src/events.rs` ve `src/lib/types.ts` içinde; `ts_contract` testi ikisini senkron tutar.

### Spotify olmadan test: mock player

`dev-tools/mock-player`, D-Bus'ta `org.mpris.MediaPlayer2.sahnemock` adıyla sahte bir oynatıcı yayınlar.

```sh
cd dev-tools/mock-player
cargo run                                   # Rick Astley, Queen, Tarkan
cargo run -- --zero-position                # Position hep 0 dönen bozuk oynatıcıyı taklit eder
cargo run -- --track "Sanatçı|Başlık|Albüm|213.5|file:///yol/kapak.png"
```

Çalışırken terminale `p` (oynat/duraklat), `n` (sonraki), `b` (önceki), `s 60` (60. saniye), `q` (çık) yazılabilir. Sahne'yi mock'a bağlamak için (Spotify açıkken bile):

```sh
SAHNE_PLAYER=sahnemock pnpm tauri dev
```

### Pencere açmadan arayüz önizleme

`preview.html`, arayüzü sahte bir backend ile normal tarayıcıda çizer:

```sh
pnpm vite --port 1430
# http://127.0.0.1:1430/preview.html?view=main&style=kinetik&at=13000
# parametreler: view=main|mini, style=klasik|kinetik|sade, at=<ms>, paused=1,
#               panel=settings, lyrics=synced|plain|loading|not_found, lang=en, static=1
```

## MPRIS verisini elle kontrol etme

```sh
playerctl -l                        # bağlı oynatıcılar (spotify görünmeli)
playerctl -p spotify metadata       # başlık, sanatçı, kapak adresi (mpris:artUrl), süre (mpris:length)
playerctl -p spotify position       # saniye cinsinden konum; çalarken artmalı
playerctl -p spotify status         # Playing / Paused
```

`position` çalarken hep `0` dönüyorsa Sahne bunu 3 saniye içinde fark eder, süreyi kendi saatiyle sayar ve ayarlarda bir uyarı gösterir. Bu modda ileri sarma Sahne üzerinden yapılırsa doğru kalır; Spotify içinden sarınca bir sonraki şarkıya kadar kayabilir.

## Elle kontrol listesi

- [ ] Spotify'da şarkı değiştir → başlık, kapak, renkler ve sözler değişir.
- [ ] Spotify'da ileri/geri sar → sözler anında yeni konuma atlar.
- [ ] Duraklat / devam et → "Duraklatıldı" rozeti çıkar/kaybolur, sözler durur/devam eder.
- [ ] Spotify'ı kapat → "Spotify açık değil" ekranı; yeniden aç → kendiliğinden bağlanır.
- [ ] İnternet bağlantısını kes, daha önce çalınmamış bir şarkı aç → "Çevrimdışı" ekranı; bağlantı gelince sözler kendiliğinden yüklenir.
- [ ] Sözü olmayan veya enstrümantal bir şarkı çal → büyük kapak + "Söz bulunamadı"/"Enstrümantal".
- [ ] Türkçe sözlü bir şarkı çal (ör. Tarkan) → ç, ğ, ı, İ, ö, ş, ü doğru görünür.
- [ ] `[` / `]` ile kaydırma yap, uygulamayı kapatıp aç → kaydırma korunur.
- [ ] `M` ile mini moda geç ve geri dön; `S` ile stilleri dene; `F` ile tam ekran.

## Ortam ışığı (OpenRGB)

Varsayılan derlemede **yoktur**. Rust OpenRGB istemcisi (`openrgb2`) GPL-2.0 lisanslıdır; MIT lisanslı Sahne'ye varsayılan olarak bağlanmaz. Açmak için:

```sh
pnpm tauri build -- --features lighting     # veya: pnpm tauri dev -- --features lighting
```

> Bu şekilde derlenen ikili dosya GPL-2.0 koşullarıyla dağıtılmalıdır.

Ardından OpenRGB'yi kur (`sudo pacman -S openrgb`, `sudo apt install openrgb` vb.), açıp **SDK Server**'ı başlat (varsayılan `127.0.0.1:6742`). Sahne'de Ayarlar → Ortam ışığı'ndan cihazları seç. Modlar: **Sabit** (paletin canlı rengi), **Gradyan** (soldan sağa 2–3 renk), **Nefes** (parlaklık yavaşça dalgalanır). Renkler 1 saniyede yumuşakça geçer; renk değişmiyorsa veri gönderilmez. OpenRGB kapalıysa Sahne sessizce bekler; çıkışta LED'leri önceki mod ve renklerine döndürür.

## Bilinen sınırlar

- **NVIDIA + Wayland:** WebKitGTK'nın DMA-BUF çizicisi "Error 71" ile çöker. Sahne NVIDIA sürücüsünü görünce `WEBKIT_DISABLE_DMABUF_RENDERER=1` ayarlar. Bunu kendin ayarlarsan Sahne dokunmaz.
- **Wayland'de mini pencere:** "Her zaman üstte" KDE'de çalışır. GNOME Wayland uygulamaların bunu yapmasına izin vermez. Wayland'de pencere konumu da uygulama tarafından kaydedilip geri yüklenemez (X11'de yapılır).
- **LRCLIB:** Topluluk verisidir. Bazı şarkılarda zamanlama kayık olabilir (`[`/`]` bunun için var), Türkçe şarkılarda kapsama daha azdır. Çok sık istekte geçici olarak sınırlar (HTTP 429). Sahne bu durumda arka planda yeniden dener ve "aranıyor" ekranında kalır. 4 satırdan az içeren kayıtlar çöp kabul edilip atlanır.
- Kelime vurgusu, Enhanced LRC yoksa tahminidir (yukarıya bak).

## Flatpak notları (ileride)

- D-Bus: `--talk-name=org.mpris.MediaPlayer2.*` (oynatıcıları okumak ve kontrol etmek için).
- Ağ: `--share=network` (LRCLIB ve kapak indirme).
- Önbellek/ayarlar Flatpak'ın XDG dizinlerine düşer; `directories` crate'i bunu kendiliğinden halleder.
- OpenRGB için: `--share=network` yeterli (yerel TCP 6742).
- NVIDIA'da `WEBKIT_DISABLE_DMABUF_RENDERER=1` manifest'te ortam değişkeni olarak da verilebilir.

## Lisans

MIT (bkz. `LICENSE`). Gömülü Manrope yazı tipi SIL Open Font License 1.1 ile dağıtılır (`src/assets/fonts/OFL.txt`). `lighting` özelliğiyle derlenen sürümler GPL-2.0'dır (yukarıya bak).
