# gow3_launcher
Launcher for God Of War 3 PC on Linux and Windows

 # ⚡ God of War® III Remastered - Linux-Windows Port & Native Launcher                                                                                                                        
                                                                                                                                                                                          
   An optimized Linux-Windows port and native graphical launcher for **God of War® III Remastered** (*CUSA01623 / CUSA01715*), featuring hardware auto-tuning, 144Hz Delta-Time synchronization, crash diagnostics, and upstream GitHub integration.                                                                                                                                     
                                                                                                                                                                                          
   [![Platform](https://img.shields.io/badge/Platform-Windows%20%7C%20Linux%20%7C%20Steam%20Deck-orange.svg)](https://kernel.org)                                                                        
   [![Launcher](https://img.shields.io/badge/Launcher-Rust%20%2F%20egui-blue.svg)](https://www.rust-lang.org)                                                                            
   [![Vulkan](https://img.shields.io/badge/Graphics-Vulkan%201.3-red.svg)](https://www.vulkan.org)                                                                                       
   [![License](https://img.shields.io/badge/License-GPLv2-green.svg)](LICENSE)        
   Website: [https://screamangel.github.io/gow3-site/](https://screamangel.github.io/gow3-site/)
                                                                                                                                                                                          
                                                                                                                                                                                      
                                                                                                                                                                                          
   ## 🌟 Key Features                                                                                                                                                                    
                                                                                                                                                                                          
   - 🦀 **Native Rust GUI Launcher (`gow3-launcher`):** Built with `eframe`/`egui`. Ultra-fast, lightweight, with a sleek game-art interface and no heavy dependencies.                                                                                                                                                                           
    - 🖥️ **Hardware Auto-Detection:** Automatically inspects GPU model, dedicated VRAM, system RAM, and display refresh rate (Hz) on launch.                                              
    - ⏱️ **144Hz / High-Hz Delta-Time Sync:** Eliminates the infamous 2x speed bug on 144Hz+ gaming displays by syncing engine delta-time with target framerates.                         
    - 🎨 **Texture Glitch Fix (Instant Readbacks):** Prevents snowy, bright, or reflection textures from corrupting into golden-yellow artifacts.                                         
    - 🎮 **Feral GameMode Integration:** Built-in toggle for `gamemoderun` to maximize CPU governor and GPU clocks during gameplay.                                                       
    - 🔍 **Smart Game Scanner & Completeness Check:** Scans drives to locate `eboot.bin` dumps, validates file integrity (`param.sfo`, elf size), and prevents launching corrupted        
  directories.                                                                                                                                                                            
    - 🚨 **Crash & Error Diagnostic Assistant:** Automatically analyzes `gow3_log.txt` on exit. If an unexpected crash occurs, it displays a plain-language diagnosis and recommended fix.
    - 🔄 **GitHub Upstream Sync & Auto-Updater:** Checks for updates in Upstream ([ScreamAngel/gow3_launcher](https://github.com/ScreamAngel/gow3_launcher)) and updates the launcher to the latest version.                                                                                                                
   - 🌐 **Multi-Language Support:** Available in **Turkish 🇹🇷**, **English 🇬🇧**, **German 🇩🇪**, and **Spanish 🇪🇸**.                                                                     

                                                                                                                                                                                       
                                                                                                                                                                                          
   ## 📋 System Requirements                                                                                                                                                             
                                                                                                                                                                                          
   | Component | Minimum | Recommended |                                                                                                                                                 
   |---|---|---|                                                                                                                                                                         
   | **OS** | Linux (Arch, Ubuntu, Fedora, SteamOS, Debian) and Windows 10-11 | Any modern 64-bit OS |                                                                                         
   | **CPU** | 6-core x86_64 processor | 8-core / 16-thread modern CPU |                                                                                                                 
   | **GPU** | Vulkan 1.3 compatible (4 GB VRAM) | Vulkan 1.3+ (6 GB+ VRAM, Nvidia / AMD) |                                                                                              
   | **RAM** | 8 GB | 16 GB+ |                                                                                                                                                           
   | **Dependencies** | Python 3, Vulkan drivers | `gamemode` (optional) |     

   ## Video
   
   You can watch the video for details.

   <p align="center">
  <a href="https://www.youtube.com/watch?v=-yGAEjdQGzE"><img width="1920" height="1080" alt="vlc_ie5PhePn2o" src="https://github.com/user-attachments/assets/190f531e-55b0-4a4a-b883-440cd615a18a" /></a>
</p>

                                                                                                                                                                                          
                                                                                                                                                                                       
                                                                                                                                                                                          
   ## 🚀 Quick Start Guide                                                                                                                                                               
                                                                                                                                                                                          
   ### 1. Installation & Setup                                                                                                                                                                   
   
   **Option A: Clone with Git**
   ```bash
   git clone https://github.com/ScreamAngel/gow3_launcher.git
   cd gow3_launcher
   ```
   - 🐧 **On Linux**: `./gow3-launcher`
   - 🪟 **On Windows**: `gow3-launcher-win.exe`

   **Option B: Download from release tab or website**
   - Extract the downloaded file from the archive. Then, depending on your operating system, select and run the appropriate file ( “gow3-launcher1” for Linux or “gow3-launcher-win.exe” for Windows).
   

   The launcher is completely standalone: it automatically detects your hardware, verifies the game dump, configures settings files, and applies all required patches without requiring external bash scripts.


   <img width="1920" height="1016" alt="gow3_launcher_win_BJ42UCuIvf" src="https://github.com/user-attachments/assets/3d57aaed-8869-446e-bdb7-89491275039d" />

                                                                                                                                                                                      
                                                                                                                                                                                          
   ### 2. Add Your Game Dump                                                                                                                                                             
   Place your decrypted PS4 God of War III Remastered game folder (`CUSA01623` or `CUSA01715` containing `eboot.bin`) into this directory, **or** keep it on any internal/external drive and select it through the Launcher.                                                                                                                                                                
                                                                                                                                                                                          
   ### 3. Launch the Game                                                                                                                                                                
   Run the launcher executable matching your current operating system:                                                                                                                                                   
                                                                                                                                                                                  
   - **Linux**: Run `./gow3-launcher` (or double-click in your file manager)
   - **Windows**: Double-click `gow3-launcher-win.exe`
   - **Direct Launch (Steam / Steam Deck / Big Picture)**: You can launch directly into the game without the launcher UI by passing `--play`:
   ```bash
   ./gow3-launcher --play
   # on Windows:
   .\gow3-launcher-win.exe --play
   ```

<img width="1920" height="1080" alt="gow3-probe_Hz51yYBACH" src="https://github.com/user-attachments/assets/651de8e7-15b9-454c-ab6d-a09dbd71f44e" />


   ### 🔄 Dual-Boot Ready
   Both `gow3-launcher` and `gow3-launcher-win.exe` live side-by-side in the same directory. If you dual-boot between Linux and Windows, you can play on either operating system seamlessly while sharing the same game dumps and configurations!
                                                                                                                                                                                          
                                                                                                                                                                                      
                                                                                                                                                                                          
   ## ⚙️ Launcher Architecture                                                                                                                                                         
                                                                                                                                                                                          
   - **`gow3-launcher`**: Standalone native Rust launcher and engine runner with embedded runtime pipelines.
   - **Zero Script Clutter**: All preparation, module linking, and patch pipelines are built directly into the launcher executable.                                                                                                                      
                                                                                                                                                                                          
                                                                                                                                                                                       
                                                                                                                                                                                          
   ## 🛠️ Troubleshooting                                                                                                                                                                 
                                                                                                                                                                                          
   - **Textures turn yellow in snow/bright areas?**  
     Ensure **"Yellow Texture Glitch Fix (Instant Readbacks)"** is checked in the launcher.
   - **Game runs twice as fast on 144Hz+ monitors?**
     Ensure **"Refresh Rate (Hz) Sync Patch"** is enabled and your target FPS matches your display.
   - **Game crashes during heavy combat or asset streaming?**  
     Check if your GPU has at least 6GB VRAM. You can adjust the VRAM Garbage Collection Budget directly in the launcher.
   - **Does the Launcher have a keyboard configuration?**
     On the “Configuration & Controls” tab, you can assign any button to a function of your choice. DualShock 4 support is available.
                                                                                                                                                                                          
                                                                                                                                                                                       
                                                                                                                                                                                          
   ## 🇹🇷 Türkçe Açıklama                                                                                                                                                                 
                                                                                                                                                                                          
   Bu proje, **God of War® III Remastered** oyununun Linux portunu modern bir grafik arayüz (GUI Launcher) ve performans optimizasyonlarıyla buluşturan açık kaynaklı bir pakettir.      
                                                                                                                                                                                          
   ### Özellikler:
   1. **Tek ve Sade Rust Launcher:** Karmaşık `.sh` scriptleri kaldırılmış, tüm pipeline doğrudan `gow3-launcher` içine gömülmüştür.
   2. **Otomatik Donanım Algılama:** GPU, VRAM, RAM ve monitör yenileme hızınızı (Hz) anında tespit eder.
   3. **144 Hz Düzeltmesi:** Yüksek yenileme hızlı monitörlerde oyunun hızlı akmasını (2x speed) engelleyen Delta-Time yaması.
   4. **Doku Bozulması Koruması:** Karlı sahnelerdeki sararma hatasını çözen Instant Readbacks yaması.
   5. **Feral GameMode:** Tek tıkla `gamemoderun` performans modu desteği.
   6. **Akıllı Hata Teşhisi:** Oyun kapanırsa log dosyasını analiz edip hatanın sebebini ve çözümünü arayüzde bildirir.
   7. **Steam / Steam Deck Desteği:** `--play` parametresi ile arayüze girmeden doğrudan oyunu başlatabilme.
   8. **Klavye Konfigürasyonu:** "Yapılandırma & Kontroller" sekmesinden tüm tuşlara klavyeden bir tuş atayabilirsiniz. Ayrıca Dualshock 4 desteği mevcuttur.
  
   
  
   ## 📜 Credits & Acknowledgments                                                                                                                                                       

   - **Original Project:** [FelipePrado0](https://github.com/FelipePrado0/gow3_pc)
   - **Upstream Linux Port & Probe:** Developed by [R-V-L](https://github.com/R-V-L/gow3_pc) and community contributors.
   - **Launcher & Hardware Tuner:** Built with Rust, [egui/eframe](https://github.com/emilk/egui).
  
   > **Disclaimer:** God of War is a registered trademark of Sony Interactive Entertainment LLC and Santa Monica Studio. This repository **does not contain any copyrighted game assets, ROMs, or binaries**. Users must supply their own legally obtained game files.
    

  ## License
   GNU GPL v2 or later ([LICENSE](https://github.com/R-V-L/gow3_pc/blob/main/LICENSE)). Third-party components keep their own licenses.
