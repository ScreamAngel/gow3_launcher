God of War III — Linux / Steam Deck

This folder IS the port. Put your game dump in it:

    <this folder>/
      bin/ scripts/ patches/ run.sh start-gow3.sh start-gow3-deck.sh gow3-deck.ini ...
      CUSA01623/        <- your game (the folder with eboot.bin)

Run:
  - Steam Deck (Desktop Mode): double-click "God of War III.desktop", or run start-gow3-deck.sh
    (it writes a tuned preset on first run: 720p, engine 60, FPS 60, FSR 1).
  - Any Linux: double-click "start-gow3.sh" (choose Execute), or from a terminal:  bash start-gow3.sh

Updating: extract a new version over this folder; it contains no CUSA01623/, so your game is
kept. Saves, shader/pipeline cache and gow3.ini go to ~/.local/share/gow3. The game folder is
found automatically (CUSA01623/ here, or any subfolder with eboot.bin); override with
GOW3_GAME_DIR. Nothing is installed system-wide; the host's Vulkan driver is used.
