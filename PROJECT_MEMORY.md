# God of War III PC Launcher — Proje Hafızası ve Mimari Özeti

> **Versiyon:** v2.1  
> **Depo:** [github.com/ScreamAngel/gow3_launcher](https://github.com/ScreamAngel/gow3_launcher) (`main` dalı)  
> **Platform Desteği:** Linux (x86_64, Wayland/Hyprland & X11) ve Windows (x86_64 MSVC)  
> **Tarih:** 07 Kasım 2026  

---

## 1. Proje Mimarisi ve Çekirdek Dosyalar

* **Launcher Kaynak Kodu (`launcher/src/`):**
  * `main.rs`: CLI başlatma argümanları (`--play`, `-p`), pencere ikonu yükleme ve eframe/egui giriş noktası.
  * `app.rs`: Launcher ana GUI arayüzü, 6 sekme (Oyun, Grafik, Yapılandırma, Kontroller, Modlar, Sistem), shader derleme ekranı ve oyun süreci izleyicisi.
  * `discord_rpc.rs`: Sıfır ek yükümlü, yerel Discord IPC istemcisi (Linux Unix Domain Socket & Windows Named Pipe), gerçek zamanlı bölüm & boss savaşı dedektörü, çoklu dil desteği (`RpcLanguage`).
  * `win_input.rs`: Windows düşük seviye fare kancası (`WH_MOUSE_LL`), 1ms `timeBeginPeriod` zamanlayıcı çözünürlüğü ve oyun penceresini öne getirme (`SetForegroundWindow`).
  * `runner.rs`: Mod bindirme (overlay) yöneticisi, Python/Rust mod montajı, oyun süreci (`gow3-probe`) çalıştırma ve log yönlendirme.
  * `config.rs`: `launcher_config.json` yönetimi, donanım optimizasyonları ve otomatik sistem dili algılama (`default_language()`).
  * `i18n.rs`: 4 dilli arayüz yerelleştirmesi (Türkçe, English, Deutsch, Español).
  * `crash_analyzer.rs`: Oyun çöktüğünde `gow3_log.txt` çıktısını analiz edip kullanıcıya çözüm sunan hata teşhis motoru.
* **Dağıtılan İkili Dosyalar (Binaries):**
  * Linux: `gow3-launcher`
  * Windows: `gow3-launcher-win.exe`
  * Alt dizin kopyaları: `gow3-linux/gow3-launcher`, `gow3-linux/gow3-launcher-win.exe`

---

## 2. Discord Rich Presence (RPC) Entegrasyonu

* **Resmi App ID:** `1440140183675867156` (*God of War III: Remastered*)
* **Resmi Varlıklar (Assets):**
  * Kapak İkonu (Large Image): `https://cdn.discordapp.com/app-icons/1440140183675867156/2cbf4c7faf6444d474ff7b32b2025ec2.png`
  * Rozet İkonu (Small Image): `https://raw.githubusercontent.com/ScreamAngel/gow3_launcher/main/gow3.png`
* **İletişim Kanalları:**
  * **Linux:** `$XDG_RUNTIME_DIR/discord-ipc-0`..`9`, `/tmp/discord-ipc-0`..`9`, Flatpak, Snap ve Vesktop (`vesktop-ipc`).
  * **Windows:** `\\.\pipe\discord-ipc-0`..`9` (Named Pipes). Olası donmaları engellemek için `PeekNamedPipe` ile asenkron kontrol mekanizması eklendi.
* **Gerçek Zamanlı Durum Tespiti:**
  * `gow3_log.txt` dosyasından gerçek çalışma zamanı WAD yüklemeleri (`WAD_STATS: WAD <NAME>` ve `/WAD/PS4_LE/<NAME>.WAD`) taranır. Statik motor sözlüğü (`LoadWadSizeData`) **yoksayılır**.
  * `user/savedata/*/CUSA01715/*.sce_sys/param.bin` SFO dosyasından anlık kayıt checkpoint'i ve zorluk derecesi çekilir.
  * Ters kronolojik (newest-first) algoritma ile oyuncunun bulunduğu son bölge veya boss savaşı anlık belirlenir.
* **Otomatik Dil Desteği (tr, en, de, es):**
  * İşletim sistemi dili (Linux'ta `$LANG`, Windows'ta `GetUserDefaultUILanguage`) veya launcher arayüz dili baz alınarak durum otomatik yerelleştirilir.
  * **Bölüm & Boss Karşılıkları:**
    * Poseidon Boss Savaşı: `Olympus Dağı (Poseidon Boss Fight)` / `Mount Olympus (Poseidon Boss Fight)`
    * Hades Boss Savaşı: `Yeraltı Dünyası (Hades Boss Fight)` / `The Underworld (Hades Boss Fight)`
    * Helios Boss Savaşı: `Olimpiya Şehri (Helios Boss Fight)` / `City of Olympia (Helios Boss Fight)`
    * Hermes Kovalamacası: `Olimpiya Şehri (Hermes Kovalamacası)` / `City of Olympia (Hermes Chase)`
    * Herkül Boss Savaşı: `Olympus Forumu (Hercules Boss Fight)` / `Forum of Olympus (Hercules Boss Fight)`
    * Kronos Boss Savaşı: `Tartarus Çukuru (Cronos Boss Fight)` / `Pit of Tartarus (Cronos Boss Fight)`
    * Hephaistos Boss Savaşı: `Demirhane (Hephaestus Boss Fight)` / `The Forge (Hephaestus Boss Fight)`
    * Skorpius Boss Savaşı: `Olimpos Mağaraları (Skorpius Boss Fight)` / `Caverns of Olympus (Skorpius Boss Fight)`
    * Zeus Boss Savaşı: `Olympus Tepesi (Zeus Boss Fight)` / `Summit of Olympus (Zeus Boss Fight)`
    * Mücadeleler: `Olympus Mücadelesi 1-7` / `Challenge of Olympus 1-7`
    * Savaş Arenası: `Savaş Arenası (Combat Arena)` / `Combat Arena`
    * Menü: `Ana Menüde` / `In Main Menu`
    * Shader Derleme: `Shader'lar Derleniyor (%X)` / `Compiling Shaders (X%)`

---

## 3. Çözülen Kritik Hatalar ve Teknik Notlar

1. **Windows Fare Tıklama Algılama Hatası:**
   * Sorun: Windows'ta sol ve sağ tık çalışmıyordu.
   * Çözüm: `win_input.rs` içindeki `WH_MOUSE_LL` kancasında PS4 SDL2 girdi haritasının beklediği F13/F14 sanal tarama kodları (`0x64` = Kare/Hafif Vuruş, `0x65` = Üçgen/Ağır Vuruş) düzeltildi.
2. **Windows Siyah Ekran / Mod Açılış Hatası:**
   * Sorun: NTFS junction oluşturma yetki hatası yüzünden Windows'ta oyun siyah ekranda takılıyordu.
   * Çözüm: `runner.rs` içinde Rust tabanlı doğrudan dosya kopyalama bindirmesi (`out/mod-game-rust-overlay`) uygulandı, yol formatı ve dosya doğrulaması eklendi.
3. **Shader Ekranı Küçülme / Kapanma / Arka Plana Düşme:**
   * Sorun: Shader derleme ekranı açıldığında oyun arka plana düşüyor veya görev çubuğuna küçülüyordu.
   * Çözüm: Oyun penceresi açıldığında shader ekranı 10 saniye boyunca arka planda tam ekran açık kalır, oyun penceresi öne getirilir (`bring_game_window_to_front`) ve süre bitiminde launcher normal moduna döner (asla küçülmez).
4. **Linux Hyprland / Wayland Tam Ekran Hatası:**
   * Sorun: Windows için eklenen pencere boyutu komutları Linux Wayland'de oyunun kenarlıksız pencere moduna geçmesini bozuyordu.
   * Çözüm: `Maximized(true)` ve ilgili `ViewportCommand` çağrıları `#[cfg(target_os = "windows")]` ile izole edildi, Linux'ta yerel `hyprctl` ve saf Wayland geometri protokolü korundu.
5. **Discord RPC'nin Sadece "Ana Menüde" Kalması:**
   * Sorun: Motorun statik `LoadWadSizeData: WAD_Shell` satırı yüzünden log döngüsü erken çıkış veriyor ve boss/bölüm adımlarını atlıyordu.
   * Çözüm: Statik sözlük filtrelendi, gerçek WAD yüklemeleri ters sırada önceliklendirildi.

---

## 4. Derleme ve Test Komutları

* **Birim Testleri (30 Test):**
  ```bash
  cargo test --manifest-path launcher/Cargo.toml
  ```
* **Linux Release Derlemesi:**
  ```bash
  cargo build --release --manifest-path launcher/Cargo.toml
  cp -f launcher/target/release/launcher gow3-launcher
  cp -f launcher/target/release/launcher gow3-linux/gow3-launcher
  ```
* **Windows Release Derlemesi (Cross-Compile):**
  ```bash
  cargo xwin build --release --manifest-path launcher/Cargo.toml --target x86_64-pc-windows-msvc
  cp -f launcher/target/x86_64-pc-windows-msvc/release/launcher.exe gow3-launcher-win.exe
  cp -f launcher/target/x86_64-pc-windows-msvc/release/launcher.exe gow3-linux/gow3-launcher-win.exe
  ```
