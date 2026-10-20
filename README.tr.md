# Sahne

[English](README.md) · **Türkçe**

[![CI](https://github.com/Tosyaliemir/sahne/actions/workflows/ci.yml/badge.svg)](https://github.com/Tosyaliemir/sahne/actions/workflows/ci.yml)
[![Coverage](https://codecov.io/gh/Tosyaliemir/sahne/graph/badge.svg)](https://codecov.io/gh/Tosyaliemir/sahne)
[![Sürüm](https://img.shields.io/github/v/release/Tosyaliemir/sahne?include_prereleases&sort=semver)](https://github.com/Tosyaliemir/sahne/releases)
[![Lisans: istisnalarla MIT](https://img.shields.io/badge/licence-MIT%20with%20exceptions-blue)](#lisans-ve-feragatnameler)
![Platformlar: Linux, Windows](https://img.shields.io/badge/platforms-Linux%20%7C%20Windows-informational)

Sahne, Spotify'da ya da başka bir masaüstü müzik oynatıcısında çalan şarkının
sözlerini senkronize ve kelime kelime animasyonlu olarak, albüm kapağının
renklerinden oluşturulan bir arka planın üzerinde gösterir.

![Bölünmüş görünüm: solda albüm kapağı, sağda kelime kelime dolan etkin söz satırı, arkada kapağın renklerinden oluşan arka plan](docs/screenshots/hero.webp)

## Özellikler

- [LRCLIB](https://lrclib.net)'ten senkronize sözler, altı stilde kelime
  kelime animasyonla.
- Albüm kapağından üretilen bir arka plan; isteğe bağlı kapak efektleri ve
  kenar ışığı.
- Spotify'la ve sistemin medya kontrollerine bilgi veren her oynatıcıyla
  çalışır (Linux'ta MPRIS, Windows'ta medya oturumları). Spotify hesabı, API
  anahtarı ya da Premium abonelik gerekmez.
- Mini oynatıcı, ekran koruyucu modu, şarkı başına zamanlama düzeltmesi ve
  çevrimdışı önbellek.
- Linux (Wayland ve X11; GNOME ve KDE Plasma) ile Windows 10 ve 11.
- Sistem diline uyan İngilizce ve Türkçe arayüz.

## Kurulum

Son sürümü [Releases](https://github.com/Tosyaliemir/sahne/releases/latest)
sayfasından indirin ya da [kaynaktan derleyin](#kaynaktan-derleme).

Her sürümde şunlar bulunur:

- **Linux, AppImage (her dağıtım):**
  `chmod +x Sahne_*.AppImage && ./Sahne_*.AppImage`
- **Linux, .deb (Debian, Ubuntu):** `sudo apt install ./Sahne_*.deb`
- **Windows:** `Sahne_<sürüm>_x64-setup.exe` dosyasını çalıştırın. Yönetici
  izni gerektirmeden yalnızca geçerli kullanıcı için kurulur. Kurulum dosyası
  henüz kod imzalı olmadığından SmartScreen "Windows bilgisayarınızı korudu"
  diyebilir; **Ek bilgi → Yine de çalıştır** seçin.

Her sürüm SHA-256 sağlama toplamlarını (`SHA256SUMS`) listeler ve bir derleme
kaynağı kanıtı taşır
(`gh attestation verify <dosya> --repo Tosyaliemir/sahne`).

## Kullanım

Spotify'da (ya da başka bir oynatıcıda) bir şarkı çalın, ardından Sahne'yi
başlatın. Sahne kısa sürede şarkıyı tanır, sözleri bulur ve sahneyi kurar.
Varsayılan olarak kapak ve sözler ekranı paylaşır; sözleri tam ekrana almak
için sözlere tıklayın ya da `L` tuşuna basın, sonra bir satıra tıklayarak
şarkıyı oraya sarın.

| Tuş         | İşlev                                                          |
| ----------- | -------------------------------------------------------------- |
| `Boşluk`    | Oynat / duraklat                                               |
| `←` / `→`   | Önceki / sonraki şarkı                                         |
| `↑` / `↓`   | Sesi aç / kıs (oynatıcı destekliyorsa)                         |
| `L`         | Söz odağı: sözler tüm ekranı kaplar / bölünmüş görünüme döner  |
| `S`         | Sonraki söz stili                                              |
| `[` / `]`   | Bu şarkının sözlerini 100 ms önce / sonra göster (hatırlanır)  |
| `F` / `Esc` | Tam ekranı aç-kapat / tam ekrandan çık                         |
| `M`         | Mini oynatıcı (küçük, masaüstü izin veriyorsa her zaman üstte) |
| `,`         | Ayarlar                                                        |

İlerleme çubuğu tıklanabilir. Alt çubuk, fare iki saniye hareket etmeyince
gizlenir (ayarlanabilir).

## Söz stilleri

| Klasik                                                                                                | Kinetik                                                                                          |
| ----------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------ |
| ![Klasik stil: etkin satır büyük ve ortada, soldan sağa doluyor](docs/screenshots/focus-classic.webp) | ![Kinetik stil: ekranın ortasında büyük harflerle tek bir kelime](docs/screenshots/kinetic.webp) |
| **Dalga**                                                                                             | **Duvar**                                                                                        |
| ![Dalga stili: kelimeler sırayla yerine yükseliyor](docs/screenshots/wave.webp)                       | ![Duvar stili: son satırlar büyük harflerle üst üste diziliyor](docs/screenshots/wall.webp)      |

- **Klasik:** etkin satır büyük ve ortadadır; kelimeler söylendikçe soldan
  sağa dolar, hafifçe yükselir ve parlar.
- **Kinetik:** her kelime tek başına ekranın ortasında belirir.
- **Sade:** hareket yok, yalnızca renk dolumu. Sistem hareketin azaltılmasını
  istiyorsa kendiliğinden seçilir.
- **Dalga:** Klasik düzen; kelimeler sırayla aşağıdan yerine yükselir.
- **Harman:** Klasik akar; hızlı bölümlerde (en az 3,5 saniye boyunca
  saniyede yaklaşık 3,4 kelimeden fazla) Kinetik'e geçer, sonra geri döner.
- **Duvar:** son satırlar, 3B'de hafifçe salınan büyük yazılı bir duvar
  oluşturur.

LRCLIB'deki sözlerin çoğu satır düzeyinde zamanlıdır. Kelime zamanları
(Enhanced LRC) yoksa satırın süresi kelimelere uzunluklarına göre dağıtılır;
yani kelime vurgusu bir tahmindir, satır geçişleri ise gerçek zamanlıdır.

## Arka planlar ve kapak efektleri

| Bulanık arka plan                                                                                            | Metal plaka                                                                                             |
| ------------------------------------------------------------------------------------------------------------ | ------------------------------------------------------------------------------------------------------- |
| ![Bulanık arka plan: sözlerin arkasında kapağın yumuşak, bulanık bir kopyası](docs/screenshots/blurred.webp) | ![Kapak, onu yansıtan fırçalanmış metal bir plakaya gömülü](docs/screenshots/cover-metal.webp)          |
| **Elektrik kenarlık**                                                                                        | **Mini oynatıcı**                                                                                       |
| ![Kapağın kenarında dolaşan çatırdayan ışık](docs/screenshots/cover-electric.webp)                           | ![Mini oynatıcı: küçük bir pencerede kapak, başlık, ilerleme ve kontroller](docs/screenshots/mini.webp) |

Arka planlar (Ayarlar → Görünüm → Arka plan):

- **Akışkan** (varsayılan): kapağın kendisi, yavaşça akar ve döner (WebGL);
  kapaktan alınan beş renkle harmanlanır. WebGL yoksa Bulanık'a döner;
  performans modu Kalite değilse yazılımsal çizimde de öyle.
- **Bulanık:** önceden bulanıklaştırılmış kapak ve üzerinde süzülen renk
  alanları.
- **Orijinal kapak:** keskin kapak, yavaşça yakınlaşır ve okunabilirlik için
  karartılır.
- **Renk:** kapağın renklerinden oluşan sakin bir geçiş.

**Performans modu** akışkan arka planı sınırlar: _Otomatik_ (30 fps, kareler
gecikirse daha düşük çözünürlük), _Kalite_ (tam hız) ve _Pil tasarrufu_
(20 fps, daha düşük çözünürlük). Duraklatılmış ya da gizli pencere hiçbir şey
çizmez.

Kapak efektleri (Ayarlar → Görünüm → Kapak efekti) renklerini kapaktan alır:

- **Metal plaka:** kapak, onu yansıtan fırçalanmış metal bir plakaya gömülür
  ve fareye doğru eğilir.
- **Elektrik kenarlık:** kapağın kenarında çatırdayan bir ışık dolaşır; fare
  yaklaşınca kıvılcımlanır, tıklayınca bir dalga yayar.

İkisi de WebGL2 ister ve sistem hareketin azaltılmasını istiyorsa kapanır.
Sistem daha yüksek kontrast istiyorsa elektrik kenarlık daha az parlar ve
titrer, metal plaka hareketli parıltılarını bırakır; Pil tasarrufu modunda
elektrik kenarlık kıvılcımsız ve 30 fps çizilir, plaka 1x çözünürlükte çizilir.
**Kenar ışığı**, ekranın kenarları boyunca akan renkler ekler.

![Pencerenin sağında açık ayarlar paneli](docs/screenshots/settings.webp)

_Ekran görüntülerindeki sözler ve kapaklar yalnızca tanıtım amaçlıdır; tüm
hakları sahiplerine aittir._

## Ekran koruyucu modu

Müzik çalarken bilgisayar bir süre (varsayılan 3 dakika) boşta kalırsa Sahne
tam ekranda öne gelir. Fareyi oynatmak ya da bir tuşa basmak pencereleri
eski hâline (gizli, küçültülmüş, mini oynatıcı…) döndürür. Süre
ayarlanabilir, mod kapatılabilir. İsteğe bağlı olarak Sahne gösterilirken
ekranın kararmasını engelleyebilir; bu, sistemin kilit ekranını da
geciktirdiği için varsayılan olarak kapalıdır.

Boşta kalma süresi, destekleyen Wayland compositor'larında `ext-idle-notify`
ile (KDE Plasma 6'da test edildi), yoksa GNOME'un Mutter IdleMonitor'ü ya da
X11'de `org.freedesktop.ScreenSaver` D-Bus arayüzüyle, Windows'ta ise son
girişin zamanından ölçülür. Müzik oynatıcılarının koyduğu uyku engelleri
dikkate alınmaz; yani mod müzik çalarken de çalışır.

## Ayarlar ve veri konumları

Ayarlar, ayarlar panelinden (`,`) değiştirilir. Dosyalar düz JSON'dur.

| İçerik                                                      | Linux                           | Windows                                |
| ----------------------------------------------------------- | ------------------------------- | -------------------------------------- |
| Ayarlar                                                     | `~/.config/sahne/settings.json` | `%APPDATA%\sahne\config\settings.json` |
| Şarkı başına söz kaydırmaları                               | `~/.config/sahne/offsets.json`  | `%APPDATA%\sahne\config\offsets.json`  |
| Söz önbelleği (bulunamayanlar 24 saat sonra yeniden aranır) | `~/.cache/sahne/lyrics/`        | `%LOCALAPPDATA%\sahne\cache\lyrics\`   |
| Kapak ve arka plan önbelleği                                | `~/.cache/sahne/art/`           | `%LOCALAPPDATA%\sahne\cache\art\`      |

Önbellekler 20 MB (sözler) ve 200 MB (kapaklar) ile sınırlıdır ve silinmeleri
güvenlidir; Ayarlar → Pencere → Önbellek boyutlarını gösterir ve temizler.

## Sorun giderme ve bilinen sınırlar

- **Wayland'de NVIDIA:** WebKitGTK'nın GPU çizicisi, NVIDIA'nın explicit sync
  özelliği yüzünden "Error 71" ile çöker. Sahne NVIDIA sürücüsünü görünce
  `__NV_DISABLE_EXPLICIT_SYNC=1` ayarlar; GPU çizimi ve 60 fps korunur. (Sık
  önerilen `WEBKIT_DISABLE_DMABUF_RENDERER=1` de çökmeyi önler, ama kareleri
  CPU üzerinden kopyaladığı için tam ekranda takılır.) Değişkeni kendiniz
  ayarlarsanız Sahne ona dokunmaz.
- **GNOME Wayland'de mini oynatıcı:** GNOME uygulamaların bir pencereyi her
  zaman üstte tutmasına izin vermez; hiçbir Wayland compositor'ı da pencere
  konumunun geri yüklenmesine izin vermez. "Her zaman üstte" KDE Plasma'da
  çalışır; konumlar X11'de geri yüklenir.
- **LRCLIB kapsamı:** LRCLIB ücretsiz, topluluk tarafından sürdürülen bir
  hizmettir. Bazı şarkılar eksik ya da biraz kaymış olabilir (`[` ve `]`
  bunun içindir); İngilizce dışındaki şarkılarda kapsam daha düşüktür. Sahne
  isteklerini aralar, hizmet yoğunken bekler ve arka planda yeniden dener.
- **Windows'ta ses:** Windows medya oturumları oynatıcının ses düzeyini
  vermez; bu yüzden orada ses kontrolü gizlenir.
- **Konumu 0 bildiren oynatıcılar:** bazı oynatıcılar konumu hep 0 bildirir.
  Sahne bunu üç saniye içinde fark eder, süreyi kendisi sayar ve ayarlarda bir
  not gösterir. Bu modda Sahne üzerinden sarmak doğru kalır; oynatıcının
  içinden sarmak bir sonraki şarkıya kadar kayabilir.

Linux'ta bir oynatıcının ne bildirdiğini `playerctl` ile görebilirsiniz:

```sh
playerctl -l                    # bağlı oynatıcılar
playerctl -p spotify metadata   # başlık, sanatçı, kapak adresi, süre
playerctl -p spotify position   # çalarken artmalı
```

Ayrıntılı log için Sahne'yi `RUST_LOG=sahne_lib=debug` ile başlatın.

## Kaynaktan derleme

Rust (`rust-toolchain.toml` içindeki sabit sürümü `rustup` kurar), Node.js
22, pnpm 10 ve platformun WebView geliştirme dosyaları gerekir; paket
listeleri için [CONTRIBUTING.md](CONTRIBUTING.md#development-setup) dosyasına
bakın.

```sh
pnpm install
pnpm tauri build                      # Linux: AppImage ve .deb
pnpm tauri build --bundles nsis       # Windows: kurulum dosyası
```

Paketler `src-tauri/target/release/bundle/` altına yazılır.

## Geliştirme

```sh
pnpm tauri dev                       # anında yeniden yüklemeyle çalıştır
pnpm lint && pnpm check && pnpm test # arayüz: ESLint/Prettier, tipler, birim testleri
cd src-tauri && cargo test           # backend testleri
```

- **Sahte oynatıcı:** `dev-tools/mock-player`, Spotify olmadan test için
  sahte bir MPRIS oynatıcısı yayınlar; Sahne'yi `SAHNE_PLAYER=sahnemock` ile
  çalıştırın.
- **Önizleme aracı:** `preview.html`, arayüzü sahte bir backend ile normal
  bir tarayıcıda çizer (`pnpm vite --port 1430`, ardından
  `http://127.0.0.1:1430/preview.html`). `pnpm screenshots` bu README'deki
  görüntüleri yeniden üretir.

Elle test listesi dahil ayrıntılar için
[CONTRIBUTING.md](CONTRIBUTING.md#manual-test-checklist).

## Mimari

Rust ile yazılmış bir backend (Tauri 2) müzik oynatıcısını izler, sözleri ve
kapağı getirir ve bir Svelte 5 arayüzüne olaylar gönderir. Servisler, olay
sözleşmesi, iş parçacığı modeli ve önbellek düzeni için
[docs/ARCHITECTURE.md](docs/ARCHITECTURE.md).

## Katkı

Hata bildirimleri, çeviriler ve pull request'ler memnuniyetle karşılanır.
Lütfen [CONTRIBUTING.md](CONTRIBUTING.md) ve
[Davranış Kuralları](CODE_OF_CONDUCT.md) belgelerini okuyun.

## Güvenlik

Güvenlik açıklarını lütfen gizli olarak bildirin; tehdit modelini de
anlatan [SECURITY.md](SECURITY.md) dosyasına bakın.

## Emeği geçenler ve teşekkürler

- David Haz'ın [React Bits](https://reactbits.dev) kütüphanesi: elektrik
  kenarlık ve metal plaka efektleri, "Electric Border" ve "Reflective Card"
  bileşenlerinden türetilmiştir (MIT + Commons Clause; bkz.
  [THIRD_PARTY_NOTICES.md](THIRD_PARTY_NOTICES.md)).
- [LRCLIB](https://lrclib.net): söz veritabanı ve API'si.
- [Manrope](https://github.com/googlefonts/manrope): gömülü yazı tipi (SIL
  Open Font License 1.1).
- [Tauri](https://tauri.app) ve [Svelte](https://svelte.dev): uygulama ve
  arayüz çatıları.
- Apple Music ve [AMLL](https://github.com/amll-dev/applemusic-like-lyrics):
  akışkan arka plan için görsel ilham. İkisinden de kod alınmamıştır.

## Lisans ve feragatnameler

Sahne MIT lisanslıdır (bkz. [LICENSE](LICENSE));
[THIRD_PARTY_NOTICES.md](THIRD_PARTY_NOTICES.md) içinde listelenen üçüncü
taraf dosyalar kendi lisanslarını korur. İki efekt modülü MIT + Commons Clause
lisanslı olduğundan Sahne bir bütün olarak tamamen OSI onaylı açık kaynak
değildir. Ayarlar → Hakkında, paketlenen tüm bağımlılıkların lisanslarını
listeler.

Sahne'nin Spotify ile bir bağlantısı yoktur ve Spotify tarafından
onaylanmamıştır. Spotify, Spotify AB'nin ticari markasıdır.

Sözler LRCLIB topluluğu tarafından sağlanır ve hakları sahiplerine aittir;
Sahne onları yalnızca gösterir ve yerel olarak önbelleğe alır.

Ekran görüntülerindeki sözler ve kapaklar yalnızca tanıtım amaçlıdır; tüm
hakları sahiplerine aittir.
