# gow3_pclauncher
Launcher for God Of War 3 PC on Linux

 # ⚡ God of War® III Remastered - Linux Port & Native Launcher                                                                                                                        
                                                                                                                                                                                          
   An optimized Linux port and native graphical launcher for **God of War® III Remastered** (*CUSA01623 / CUSA01715*), featuring hardware auto-tuning, 144Hz Delta-Time synchronization, crash diagnostics, and upstream GitHub integration.                                                                                                                                     
                                                                                                                                                                                          
   [![Platform](https://img.shields.io/badge/Platform-Linux%20%7C%20Steam%20Deck-orange.svg)](https://kernel.org)                                                                        
   [![Launcher](https://img.shields.io/badge/Launcher-Rust%20%2F%20egui-blue.svg)](https://www.rust-lang.org)                                                                            
   [![Vulkan](https://img.shields.io/badge/Graphics-Vulkan%201.3-red.svg)](https://www.vulkan.org)                                                                                       
   [![License](https://img.shields.io/badge/License-GPLv2-green.svg)](LICENSE)                                                                                                           
                                                                                                                                                                                          
                                                                                                                                                                                      
                                                                                                                                                                                          
   ## 🌟 Key Features                                                                                                                                                                    
                                                                                                                                                                                          
   - 🦀 **Native Rust GUI Launcher (`gow3-launcher`):** Built with `eframe`/`egui`. Ultra-fast, lightweight (~17 MB standalone), with a sleek game-art interface and no heavy            
  dependencies.                                                                                                                                                                           
    - 🖥️ **Hardware Auto-Detection:** Automatically inspects GPU model, dedicated VRAM, system RAM, and display refresh rate (Hz) on launch.                                              
    - ⏱️ **144Hz / High-Hz Delta-Time Sync:** Eliminates the infamous 2x speed bug on 144Hz+ gaming displays by syncing engine delta-time with target framerates.                         
    - 🎨 **Texture Glitch Fix (Instant Readbacks):** Prevents snowy, bright, or reflection textures from corrupting into golden-yellow artifacts.                                         
    - 🎮 **Feral GameMode Integration:** Built-in toggle for `gamemoderun` to maximize CPU governor and GPU clocks during gameplay.                                                       
    - 🔍 **Smart Game Scanner & Completeness Check:** Scans drives to locate `eboot.bin` dumps, validates file integrity (`param.sfo`, elf size), and prevents launching corrupted        
  directories.                                                                                                                                                                            
    - 🚨 **Crash & Error Diagnostic Assistant:** Automatically analyzes `gow3_log.txt` on exit. If an unexpected crash occurs, it displays a plain-language diagnosis and recommended fix.
    - 🔄 **GitHub Upstream Sync & Auto-Updater:** Checks upstream ([R-V-L/gow3_pc](https://github.com/R-V-L/gow3_pc)) for updates. Backs up `run.sh` to `run.sh.bak` and re-applies all   
  your custom directory paths, 144Hz patches, and VRAM budgets seamlessly.                                                                                                                
    - 🌐 **Multi-Language Support:** Available in **Turkish 🇹🇷**, **English 🇬🇧**, **German 🇩🇪**, and **Spanish 🇪🇸**.                                                                      
                                                                                                                                                                                          
                                                                                                                                                                                       
                                                                                                                                                                                          
   ## 📋 System Requirements                                                                                                                                                             
                                                                                                                                                                                          
   | Component | Minimum | Recommended |                                                                                                                                                 
   |---|---|---|                                                                                                                                                                         
   | **OS** | Linux (Arch, Ubuntu, Fedora, SteamOS, Debian) | Any modern 64-bit Linux distro |                                                                                           
   | **CPU** | 6-core x86_64 processor | 8-core / 16-thread modern CPU |                                                                                                                 
   | **GPU** | Vulkan 1.3 compatible (4 GB VRAM) | Vulkan 1.3+ (6 GB+ VRAM, Nvidia / AMD) |                                                                                              
   | **RAM** | 8 GB | 16 GB+ |                                                                                                                                                           
   | **Dependencies** | Python 3, Vulkan drivers | `gamemode` (optional) |                                                                                                               
                                                                                                                                                                                          
                                                                                                                                                                                       
                                                                                                                                                                                          
   ## 🚀 Quick Start Guide                                                                                                                                                               
                                                                                                                                                                                          
   ### 1. Installation                                                                                                                                                                   
   Download this repository:                                                                                                                                                    
   Extract the zip file into a folder.
   You can then launch the game using the "gow3-launcher" file found inside.
   This launcher has been designed to be simple and user-friendly, making things easier for you.
   It automatically configures the settings files based on your system, detects the game's installation location, and allows you to apply patches. Once you have finished the setup, you can start playing.
   If any issues arise, you can access the log files directly through the launcher to investigate.
   The launcher also checks for any available updates. Enjoy the game!   

   OR

   ### Clone the Repository
   bash                                                                               
   git clone https://github.com/ScreamAngel/gow3_pclauncher.git                          
   cd gow3_pclauncher                                                                    
                                                                                          
  #### 2. Make Sure Scripts & Launcher Are Executable                                     
                                                                                          
   chmod +x gow3-launcher run.sh launcher.sh start-gow3.sh    
                                                                                                                                                                                      
                                                                                                                                                                                          
   ### 2. Add Your Game Dump                                                                                                                                                             
   Place your decrypted PS4 God of War III Remastered game folder (`CUSA01623` or `CUSA01715` containing `eboot.bin`) into this directory, **or** keep it on an external drive and select it through the Launcher.                                                                                                                                                                
                                                                                                                                                                                          
   ### 3. Launch the Game     

   ./gow3-launcher (Or run ./launcher.sh)

   OR
   
   Run the native launcher executable:                                                                                                                                                   
                                                                                                                                                                                  
   double-click `gow3-launcher` in folder.                                                                                                                                                                      
                                                                                                                                                                                       
   *(Alternatively, you can run `launcher.sh`)*                                                                                                 
                                                                                                                                                                                          
   The launcher will detect your hardware, verify your game files, apply the 144Hz sync and texture fixes, and you're ready to play!                                                     
                                                                                                                                                                                          
                                                                                                                                                                                      
                                                                                                                                                                                          
   ## ⚙️ Included Launch Scripts                                                                                                                                                         
                                                                                                                                                                                          
   - **`gow3-launcher`**: The primary standalone Rust GUI launcher.                                                                                                                      
   - **`launcher.sh`**: Helper bash script to execute the launcher.                                                                                                                      
   - **`run.sh`**: The underlying engine runner script with tuned environment variables.                                                                                                 
   - **`start-gow3.sh`**: Headless launcher script for standard desktop usage.
   - **`start-gow3-deck.sh`**: Optimized startup script for Steam Deck / SteamOS (720p / 60 FPS profile).
  
    
  
   ## 🛠️ Troubleshooting                                                                                                                                                                 
  
   - **Textures turn yellow in snow/bright areas?**  
     Ensure **"Yellow Texture Glitch Fix (Instant Readbacks)"** is checked in the launcher.
   - **Game runs twice as fast on 144Hz+ monitors?**
   - Ensure **"Refresh Rate (Hz) Sync Patch"** is enabled and your target FPS matches your display.
   - **Game crashes during heavy combat or asset streaming?**  
     Check if your GPU has at least 6GB VRAM. You can adjust the VRAM Garbage Collection Budget directly in the launcher.
  
    
  
   ## 🇹🇷 Türkçe Açıklama                                                                                                                                                                 
  
   Bu proje, **God of War® III Remastered** oyununun Linux portunu modern bir grafik arayüz (GUI Launcher) ve performans optimizasyonlarıyla buluşturan açık kaynaklı bir pakettir.      
  
   ### Özellikler:
   1. **Rust ile Yazılmış Hafif Launcher:** Donanımınızı (GPU, VRAM, RAM, Monitör Hz) otomatik algılar.
   2. **144 Hz Düzeltmesi:** Yüksek yenileme hızlı monitörlerde oyunun hızlı akmasını (2x speed) engelleyen Delta-Time yaması.
   3. **Doku Bozulması Koruması:** Karlı sahnelerdeki sararma hatasını çözen Instant Readbacks yaması.
   4. **Feral GameMode:** Tek tıkla `gamemoderun` performans modu desteği.
   5. **Akıllı Hata Teşhisi:** Oyun kapanırsa log dosyasını analiz edip hatanın sebebini ve çözümünü pop-up olarak bildirir.
   6. **GitHub Güncelleme Entegrasyonu:** [R-V-L/gow3_pc](https://github.com/R-V-L/gow3_pc) üzerindeki güncellemeleri kontrol eder, `run.sh` yedeğini alıp ayarlarınızı bozmadan entegre eder.
  
   
  
   ## 📜 Credits & Acknowledgments                                                                                                                                                       

   - **Original Project:** https://github.com/FelipePrado0/gow3_pc
   - **Upstream Linux Port & Probe:** Developed by [R-V-L](https://github.com/R-V-L/gow3_pc) and community contributors.
   - **Launcher & Hardware Tuner:** Built with Rust, [egui/eframe](https://github.com/emilk/egui).
  
   > **Disclaimer:** God of War is a registered trademark of Sony Interactive Entertainment LLC and Santa Monica Studio. This repository **does not contain any copyrighted game assets, ROMs, or binaries**. Users must supply their own legally obtained game files.
    

   #Licence
   GNU GPL v2 or later ([LICENSE](https://github.com/R-V-L/gow3_pc/blob/main/LICENSE)). Third-party components keep their own licenses.
