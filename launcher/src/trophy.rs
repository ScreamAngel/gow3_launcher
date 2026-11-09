// SPDX-License-Identifier: MIT
// God of War III Remastered - PS4 Trophy Management & Toast Notification System

use eframe::egui::{self, Color32, CornerRadius, Pos2, Rect, Stroke, Vec2};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet, VecDeque};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

pub const TROPHY_SOUND_WAV: &[u8] = include_bytes!("../assets/trophy.wav");

pub const ICON_BRONZE: &[u8] = include_bytes!("../assets/bronze.png");
pub const ICON_SILVER: &[u8] = include_bytes!("../assets/silver.png");
pub const ICON_GOLD: &[u8] = include_bytes!("../assets/gold.png");
pub const ICON_PLATINUM: &[u8] = include_bytes!("../assets/platinum.png");

macro_rules! trop_icon {
    ($num:literal) => {
        include_bytes!(concat!("../assets/trophies/TROP", $num, ".PNG"))
    };
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum TrophyGrade {
    Platinum,
    Gold,
    Silver,
    Bronze,
}

impl TrophyGrade {
    pub fn name_en(&self) -> &'static str {
        match self {
            Self::Platinum => "Platinum",
            Self::Gold => "Gold",
            Self::Silver => "Silver",
            Self::Bronze => "Bronze",
        }
    }

    pub fn name_tr(&self) -> &'static str {
        match self {
            Self::Platinum => "Platin",
            Self::Gold => "Altın",
            Self::Silver => "Gümüş",
            Self::Bronze => "Bronz",
        }
    }

    pub fn badge_color(&self) -> Color32 {
        match self {
            Self::Platinum => Color32::from_rgb(175, 225, 255),
            Self::Gold => Color32::from_rgb(255, 210, 60),
            Self::Silver => Color32::from_rgb(215, 225, 235),
            Self::Bronze => Color32::from_rgb(215, 145, 95),
        }
    }

    pub fn icon_bytes(&self) -> &'static [u8] {
        match self {
            Self::Platinum => ICON_PLATINUM,
            Self::Gold => ICON_GOLD,
            Self::Silver => ICON_SILVER,
            Self::Bronze => ICON_BRONZE,
        }
    }
}

pub struct TrophyDefinition {
    pub id: u32,
    pub name_en: &'static str,
    pub name_tr: &'static str,
    pub desc_en: &'static str,
    pub desc_tr: &'static str,
    pub grade: TrophyGrade,
    pub hidden: bool,
    pub icon_bytes: &'static [u8],
}

pub static ALL_TROPHIES: [TrophyDefinition; 36] = [
    TrophyDefinition {
        id: 0,
        name_en: "King of the Hill",
        name_tr: "Tepenin Kralı",
        desc_en: "Unlock all trophies",
        desc_tr: "Tüm kupaların kilidini aç",
        grade: TrophyGrade::Platinum,
        hidden: true,
        icon_bytes: trop_icon!("000"),
    },
    TrophyDefinition {
        id: 1,
        name_en: "Releasing the Floodgates",
        name_tr: "Baraj Kapakları Açılsın",
        desc_en: "Kill Poseidon",
        desc_tr: "Poseidon'u öldür",
        grade: TrophyGrade::Bronze,
        hidden: true,
        icon_bytes: trop_icon!("001"),
    },
    TrophyDefinition {
        id: 2,
        name_en: "Burnt to a Crisp",
        name_tr: "Çıtır Çıtır Kızarmış",
        desc_en: "Acquire the Bow of Apollo",
        desc_tr: "Apollo'nun yayını ele geçir",
        grade: TrophyGrade::Bronze,
        hidden: true,
        icon_bytes: trop_icon!("002"),
    },
    TrophyDefinition {
        id: 3,
        name_en: "Mr. Hand",
        name_tr: "El Ele Kol Kola",
        desc_en: "Discover the secret 'Hades Arm' room",
        desc_tr: "Gizli 'Hades'in Kolu' odasını keşfet",
        grade: TrophyGrade::Bronze,
        hidden: false,
        icon_bytes: trop_icon!("003"),
    },
    TrophyDefinition {
        id: 4,
        name_en: "Shine Lord",
        name_tr: "Işık Saçıyorsun",
        desc_en: "Kill Helios",
        desc_tr: "Helios'u öldür",
        grade: TrophyGrade::Bronze,
        hidden: true,
        icon_bytes: trop_icon!("004"),
    },
    TrophyDefinition {
        id: 5,
        name_en: "Shoe Delivery",
        name_tr: "Ayakkabı Teslimatı",
        desc_en: "Kill Hermes and acquire his Boots",
        desc_tr: "Hermes'i öldürüp çizmelerini ele geçir",
        grade: TrophyGrade::Bronze,
        hidden: true,
        icon_bytes: trop_icon!("005"),
    },
    TrophyDefinition {
        id: 6,
        name_en: "I didn’t do it... But I wish I did!",
        name_tr: "Ben Yapmadım... Keşke Yapsaydım!",
        desc_en: "Kill the Poseidon Princess",
        desc_tr: "Poseidon'un Prensesini öldür",
        grade: TrophyGrade::Bronze,
        hidden: true,
        icon_bytes: trop_icon!("006"),
    },
    TrophyDefinition {
        id: 7,
        name_en: "Ladies Man",
        name_tr: "Kadınların Gözdesi",
        desc_en: "Successfully entertain Aphrodite",
        desc_tr: "Afrodit'i zevkten bulutlara çıkar",
        grade: TrophyGrade::Bronze,
        hidden: true,
        icon_bytes: trop_icon!("007"),
    },
    TrophyDefinition {
        id: 8,
        name_en: "Handy Man",
        name_tr: "Becerikli Kratos",
        desc_en: "Sever Gaia’s hand",
        desc_tr: "Gaia'nın elini kopar",
        grade: TrophyGrade::Bronze,
        hidden: true,
        icon_bytes: trop_icon!("008"),
    },
    TrophyDefinition {
        id: 9,
        name_en: "Open Sesame",
        name_tr: "Açıl Susam Açıl",
        desc_en: "Open the Gates of Tisiphone",
        desc_tr: "Tisiphone'un Kapıları'nı aç",
        grade: TrophyGrade::Bronze,
        hidden: true,
        icon_bytes: trop_icon!("009"),
    },
    TrophyDefinition {
        id: 10,
        name_en: "Rescue Me",
        name_tr: "Kurtar Beni",
        desc_en: "Save Pandora",
        desc_tr: "Pandora'yı kurtar",
        grade: TrophyGrade::Bronze,
        hidden: true,
        icon_bytes: trop_icon!("010"),
    },
    TrophyDefinition {
        id: 11,
        name_en: "Three Wise Men",
        name_tr: "Üç Bilge Adam",
        desc_en: "Solve the Three Judges",
        desc_tr: "Üç Yargıç bulmacasını çöz",
        grade: TrophyGrade::Bronze,
        hidden: true,
        icon_bytes: trop_icon!("011"),
    },
    TrophyDefinition {
        id: 12,
        name_en: "Hit Man",
        name_tr: "Vur Kaç",
        desc_en: "Perform a 1000-hit combo",
        desc_tr: "1000 vuruşluk bir kombo gerçekleştir",
        grade: TrophyGrade::Bronze,
        hidden: false,
        icon_bytes: trop_icon!("012"),
    },
    TrophyDefinition {
        id: 13,
        name_en: "Rip One!",
        name_tr: "Parçala!",
        desc_en: "Rip apart 1 Olympus Sentry",
        desc_tr: "1 Olimpos Gözcüsünü ikiye ayır",
        grade: TrophyGrade::Bronze,
        hidden: false,
        icon_bytes: trop_icon!("013"),
    },
    TrophyDefinition {
        id: 14,
        name_en: "Nice Tan",
        name_tr: "Güzel Bronzluk",
        desc_en: "Blind 100 enemies with the Head of Helios",
        desc_tr: "Helios'un Başı ile 100 düşmanı kör et",
        grade: TrophyGrade::Bronze,
        hidden: false,
        icon_bytes: trop_icon!("014"),
    },
    TrophyDefinition {
        id: 15,
        name_en: "Obedience School",
        name_tr: "İtaat Eğitimi",
        desc_en: "Deliver 50 kicks to Hounds",
        desc_tr: "Yeraltı tazılarına 50 tekme at",
        grade: TrophyGrade::Bronze,
        hidden: false,
        icon_bytes: trop_icon!("015"),
    },
    TrophyDefinition {
        id: 16,
        name_en: "It’s getting hot in here...",
        name_tr: "Hava Isınmaya Başladı...",
        desc_en: "Burn 100 enemies with the Bow of Apollo",
        desc_tr: "Apollo'nun Yayı ile 100 düşmanı yak",
        grade: TrophyGrade::Bronze,
        hidden: false,
        icon_bytes: trop_icon!("016"),
    },
    TrophyDefinition {
        id: 17,
        name_en: "Souled Out",
        name_tr: "Ruhların Efendisi",
        desc_en: "Summon every Soul with the Claws of Hades",
        desc_tr: "Hades'in Pençeleri ile her Ruh türünü çağır",
        grade: TrophyGrade::Bronze,
        hidden: false,
        icon_bytes: trop_icon!("017"),
    },
    TrophyDefinition {
        id: 18,
        name_en: "Bloody Hell",
        name_tr: "Kan Banyosu",
        desc_en: "Cover Kratos in 500 buckets of blood by killing the enemies",
        desc_tr: "Düşmanları öldürerek Kratos'u 500 kova dolusu kanla kapla",
        grade: TrophyGrade::Bronze,
        hidden: false,
        icon_bytes: trop_icon!("018"),
    },
    TrophyDefinition {
        id: 19,
        name_en: "No Guts, No Glory",
        name_tr: "Bağırsak Söken",
        desc_en: "Gut 3 Centaur Generals",
        desc_tr: "3 Sentor Generalinin karnını deş",
        grade: TrophyGrade::Bronze,
        hidden: false,
        icon_bytes: trop_icon!("019"),
    },
    TrophyDefinition {
        id: 20,
        name_en: "Hitting Your Stride",
        name_tr: "Formunu Yakalamak",
        desc_en: "Upgrade any weapon to the next level",
        desc_tr: "Herhangi bir silahı bir üst seviyeye yükselt",
        grade: TrophyGrade::Bronze,
        hidden: false,
        icon_bytes: trop_icon!("020"),
    },
    TrophyDefinition {
        id: 21,
        name_en: "Sibling Rivalry",
        name_tr: "Kardeş Rekabeti",
        desc_en: "Kill Hercules",
        desc_tr: "Herkül'ü öldür",
        grade: TrophyGrade::Silver,
        hidden: true,
        icon_bytes: trop_icon!("021"),
    },
    TrophyDefinition {
        id: 22,
        name_en: "Titan Slayer",
        name_tr: "Titan Katili",
        desc_en: "Kill Cronos",
        desc_tr: "Kronos'u öldür",
        grade: TrophyGrade::Silver,
        hidden: true,
        icon_bytes: trop_icon!("022"),
    },
    TrophyDefinition {
        id: 23,
        name_en: "Retribution",
        name_tr: "Hak Edilen Ceza",
        desc_en: "Kill Zeus",
        desc_tr: "Zeus'u öldür",
        grade: TrophyGrade::Silver,
        hidden: true,
        icon_bytes: trop_icon!("023"),
    },
    TrophyDefinition {
        id: 24,
        name_en: "Hooker",
        name_tr: "Kancacı",
        desc_en: "Kill Hades",
        desc_tr: "Hades'i öldür",
        grade: TrophyGrade::Silver,
        hidden: true,
        icon_bytes: trop_icon!("024"),
    },
    TrophyDefinition {
        id: 25,
        name_en: "Freezer Burn",
        name_tr: "Donma Isırığı",
        desc_en: "Acquire the Boreas Icestorm",
        desc_tr: "Boreas Buz Fırtınası'nı ele geçir",
        grade: TrophyGrade::Silver,
        hidden: true,
        icon_bytes: trop_icon!("025"),
    },
    TrophyDefinition {
        id: 26,
        name_en: "Seeing things from a different perspective",
        name_tr: "Farklı Bir Bakış Açısı",
        desc_en: "Solve Hera's Gardens",
        desc_tr: "Hera'nın Bahçesi bulmacasını çöz",
        grade: TrophyGrade::Silver,
        hidden: true,
        icon_bytes: trop_icon!("026"),
    },
    TrophyDefinition {
        id: 27,
        name_en: "Eye Candy",
        name_tr: "Göz Ziyafeti",
        desc_en: "Collect all of the Gorgon Eyes",
        desc_tr: "Tüm Gorgon Gözleri'ni topla",
        grade: TrophyGrade::Silver,
        hidden: false,
        icon_bytes: trop_icon!("027"),
    },
    TrophyDefinition {
        id: 28,
        name_en: "Feather Plucker",
        name_tr: "Tüy Yolucu",
        desc_en: "Collect all of the Phoenix Feathers",
        desc_tr: "Tüm Anka Kuşu Tüyleri'ni topla",
        grade: TrophyGrade::Silver,
        hidden: false,
        icon_bytes: trop_icon!("028"),
    },
    TrophyDefinition {
        id: 29,
        name_en: "Are You Horny to Win?",
        name_tr: "Boynuz Koleksiyoncusu",
        desc_en: "Collect all of the Minotaur Horns",
        desc_tr: "Tüm Minotaur Boynuzları'nı topla",
        grade: TrophyGrade::Silver,
        hidden: false,
        icon_bytes: trop_icon!("029"),
    },
    TrophyDefinition {
        id: 30,
        name_en: "Maxed Out!",
        name_tr: "Formunun Zirvesinde!",
        desc_en: "Completely upgrade all weapons",
        desc_tr: "Tüm silahları son seviyeye kadar yükselt",
        grade: TrophyGrade::Silver,
        hidden: false,
        icon_bytes: trop_icon!("030"),
    },
    TrophyDefinition {
        id: 31,
        name_en: "Vengeance Complete",
        name_tr: "İntikam Tamamlandı",
        desc_en: "Beat the game",
        desc_tr: "Oyunu tamamla",
        grade: TrophyGrade::Gold,
        hidden: false,
        icon_bytes: trop_icon!("031"),
    },
    TrophyDefinition {
        id: 32,
        name_en: "Up to the Challenge",
        name_tr: "Meydan Okumaya Değer",
        desc_en: "Beat the Challenge of Olympus",
        desc_tr: "Olimpos Mücadelesi'ni tamamla",
        grade: TrophyGrade::Gold,
        hidden: false,
        icon_bytes: trop_icon!("032"),
    },
    TrophyDefinition {
        id: 33,
        name_en: "Unhuman",
        name_tr: "İnsanüstü",
        desc_en: "Beat Titan Mode",
        desc_tr: "Oyunu Titan Modunda tamamla",
        grade: TrophyGrade::Gold,
        hidden: false,
        icon_bytes: trop_icon!("033"),
    },
    TrophyDefinition {
        id: 34,
        name_en: "Priceless",
        name_tr: "Paha Biçilemez",
        desc_en: "Collect all of the ‘Godly Possessions’",
        desc_tr: "Tüm Tanrısal Eşyaları topla",
        grade: TrophyGrade::Gold,
        hidden: false,
        icon_bytes: trop_icon!("034"),
    },
    TrophyDefinition {
        id: 35,
        name_en: "aMAZEd",
        name_tr: "Şaşırtıcı Labirent",
        desc_en: "Beat the Labyrinth without dying or failing",
        desc_tr: "Labirenti hiç ölmeden veya başarısız olmadan tamamla",
        grade: TrophyGrade::Gold,
        hidden: true,
        icon_bytes: trop_icon!("035"),
    },
];

pub fn get_trophy_def(id: u32) -> Option<&'static TrophyDefinition> {
    ALL_TROPHIES.iter().find(|t| t.id == id)
}

pub fn format_timestamp(ts: u64) -> String {
    // Converts unix timestamp to "DD.MM.YYYY - HH:MM" (Turkey UTC+3 / Local)
    let local_ts = ts + 3 * 3600;
    let seconds_in_day = local_ts % 86400;
    let hours = (seconds_in_day / 3600) % 24;
    let minutes = (seconds_in_day % 3600) / 60;
    let days = (local_ts / 86400) as i64 + 719468;
    let era = if days >= 0 { days } else { days - 146096 } / 146097;
    let doe = (days - era * 146097) as u32;
    let yoe = (doe - doe / 1024 + doe / 1461 - doe / 146096) / 365;
    let mut y = (yoe as i64) + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    if m <= 2 {
        y += 1;
    }
    format!("{:02}.{:02}.{:04} - {:02}:{:02}", d, m, y, hours, minutes)
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrophyItemState {
    pub unlocked: bool,
    #[serde(default)]
    pub unlocked_at: Option<u64>,
    #[serde(default)]
    pub unlocked_at_formatted: Option<String>,
}

impl Default for TrophyItemState {
    fn default() -> Self {
        Self {
            unlocked: false,
            unlocked_at: None,
            unlocked_at_formatted: None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrophyStateDatabase {
    pub title_id: String,
    pub last_updated: u64,
    pub trophies: HashMap<u32, TrophyItemState>,
}

impl Default for TrophyStateDatabase {
    fn default() -> Self {
        let mut trophies = HashMap::new();
        for t in ALL_TROPHIES.iter() {
            trophies.insert(t.id, TrophyItemState::default());
        }
        Self {
            title_id: "CUSA01715".to_string(),
            last_updated: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs(),
            trophies,
        }
    }
}

impl TrophyStateDatabase {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn load(base_dir: &Path) -> Self {
        let mut db = Self::default();

        let candidates = [
            base_dir.join("trophies_state.json"),
            base_dir.join("user/trophies_state.json"),
        ];

        let mut loaded = false;
        for path in &candidates {
            if path.is_file() {
                if let Ok(content) = fs::read_to_string(path) {
                    if let Ok(parsed) = serde_json::from_str::<TrophyStateDatabase>(&content) {
                        for (k, v) in parsed.trophies {
                            db.trophies.insert(k, v);
                        }
                        db.last_updated = parsed.last_updated;
                        loaded = true;
                        break;
                    }
                }
            }
        }

        // Also check if any shadPS4 / RPCS3 save XML files exist with unlockstate="true"
        let home_trophies = [
            base_dir.join("user/home/1/trophy/NPWR08268_00.xml"),
            base_dir.join("user/trophy/NPWR08268_00.xml"),
            base_dir.join("user/home/1/trophy/CUSA01715.xml"),
            base_dir.join("user/trophy/CUSA01715.xml"),
        ];
        for xml_file in &home_trophies {
            if xml_file.is_file() {
                if let Ok(xml_content) = fs::read_to_string(xml_file) {
                    db.import_from_xml(&xml_content);
                }
            }
        }

        if !loaded {
            db.save(base_dir);
        }

        db
    }

    pub fn import_from_xml(&mut self, xml: &str) {
        for line in xml.lines() {
            if line.contains("<trophy") && line.contains("unlockstate=\"true\"") {
                if let Some(id_str) = extract_attribute(line, "id") {
                    if let Ok(id) = id_str.parse::<u32>() {
                        if id <= 35 {
                            let entry = self.trophies.entry(id).or_default();
                            if !entry.unlocked {
                                entry.unlocked = true;
                                let ts = extract_attribute(line, "timestamp")
                                    .and_then(|s| s.parse::<u64>().ok())
                                    .unwrap_or_else(|| {
                                        SystemTime::now()
                                            .duration_since(UNIX_EPOCH)
                                            .unwrap_or_default()
                                            .as_secs()
                                    });
                                entry.unlocked_at = Some(ts);
                                entry.unlocked_at_formatted = Some(format_timestamp(ts));
                            }
                        }
                    }
                }
            }
        }
    }

    pub fn save(&self, base_dir: &Path) {
        if let Ok(json) = serde_json::to_string_pretty(self) {
            let primary = base_dir.join("trophies_state.json");
            let _ = fs::write(&primary, &json);

            let user_dir = base_dir.join("user");
            if user_dir.is_dir() {
                let secondary = user_dir.join("trophies_state.json");
                let _ = fs::write(&secondary, &json);
            }
        }
    }

    pub fn unlock(&mut self, id: u32, timestamp: u64) -> bool {
        if id > 35 {
            return false;
        }
        let entry = self.trophies.entry(id).or_default();
        if entry.unlocked {
            return false; // Already unlocked
        }
        entry.unlocked = true;
        entry.unlocked_at = Some(timestamp);
        entry.unlocked_at_formatted = Some(format_timestamp(timestamp));
        self.last_updated = timestamp;
        true
    }

    pub fn is_unlocked(&self, id: u32) -> bool {
        self.trophies.get(&id).map(|s| s.unlocked).unwrap_or(false)
    }

    pub fn get_state(&self, id: u32) -> TrophyItemState {
        self.trophies.get(&id).cloned().unwrap_or_default()
    }

    pub fn total_count(&self) -> usize {
        ALL_TROPHIES.len()
    }

    pub fn unlocked_count(&self) -> usize {
        self.trophies.values().filter(|s| s.unlocked).count()
    }

    pub fn percentage(&self) -> f32 {
        let total = self.total_count() as f32;
        if total == 0.0 {
            0.0
        } else {
            (self.unlocked_count() as f32 / total) * 100.0
        }
    }

    pub fn unlocked_by_grade(&self, grade: TrophyGrade) -> usize {
        ALL_TROPHIES
            .iter()
            .filter(|t| t.grade == grade && self.is_unlocked(t.id))
            .count()
    }

    pub fn total_by_grade(&self, grade: TrophyGrade) -> usize {
        ALL_TROPHIES.iter().filter(|t| t.grade == grade).count()
    }

    pub fn is_mastered(&self) -> bool {
        self.unlocked_count() >= self.total_count()
    }
}

fn extract_attribute<'a>(line: &'a str, attr: &str) -> Option<&'a str> {
    let pattern = format!("{}=\"", attr);
    if let Some(pos) = line.find(&pattern) {
        let start = pos + pattern.len();
        if let Some(end) = line[start..].find('"') {
            return Some(&line[start..start + end]);
        }
    }
    None
}

// ---------------------------------------------------------------------------
// AUDIO PLAYBACK SYSTEM
// ---------------------------------------------------------------------------

pub fn play_trophy_sound(_base_dir: &Path) {
    #[cfg(target_os = "windows")]
    {
        crate::win_input::play_sound_memory(TROPHY_SOUND_WAV);
    }

    #[cfg(not(target_os = "windows"))]
    {
        // On Linux, use custom_trophy/trophy.wav or /tmp/gow3_trophy.wav with pw-play/paplay/aplay
        let sound_path = _base_dir.join("user/custom_trophy/trophy.wav");
        let path_to_play = if sound_path.is_file() {
            sound_path
        } else {
            let tmp = std::env::temp_dir().join("gow3_trophy_pop.wav");
            if !tmp.is_file() {
                let _ = fs::write(&tmp, TROPHY_SOUND_WAV);
            }
            tmp
        };

        std::thread::spawn(move || {
            let players = ["pw-play", "paplay", "aplay"];
            for player in &players {
                if let Ok(mut child) = std::process::Command::new(player)
                    .arg(&path_to_play)
                    .stdout(std::process::Stdio::null())
                    .stderr(std::process::Stdio::null())
                    .spawn()
                {
                    let _ = child.wait();
                    return;
                }
            }
        });
    }
}

// ---------------------------------------------------------------------------
// NOTIFICATION & TOAST MANAGER
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct ActiveToast {
    pub trophy_id: u32,
    pub grade: TrophyGrade,
    pub title: String,
    pub start_time: Instant,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TrophyFilter {
    All,
    Earned,
    Locked,
    Secret,
}

pub struct TrophyManager {
    pub db: TrophyStateDatabase,
    pub queue: VecDeque<u32>,
    pub active_toast: Option<ActiveToast>,
    pub revealed_secrets: HashSet<u32>,
    pub base_dir: PathBuf,
    pub filter: TrophyFilter,
    pub search_query: String,
}

impl TrophyManager {
    pub fn new(base_dir: &Path) -> Self {
        let db = TrophyStateDatabase::load(base_dir);
        Self {
            db,
            queue: VecDeque::new(),
            active_toast: None,
            revealed_secrets: HashSet::new(),
            base_dir: base_dir.to_path_buf(),
            filter: TrophyFilter::All,
            search_query: String::new(),
        }
    }

    pub fn unlock_and_notify(&mut self, id: u32, lang: &str) -> bool {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        if self.db.unlock(id, now) {
            self.db.save(&self.base_dir);

            // Play sound immediately on unlock
            play_trophy_sound(&self.base_dir);

            // Add to FIFO queue
            self.queue.push_back(id);

            // Check if platinum (id 0) should be unlocked (if all 35 other trophies unlocked)
            if id != 0 && !self.db.is_unlocked(0) {
                let non_plat_unlocked = (1..=35).all(|i| self.db.is_unlocked(i));
                if non_plat_unlocked {
                    self.db.unlock(0, now + 1);
                    self.db.save(&self.base_dir);
                    self.queue.push_back(0);
                }
            }

            self.advance_queue(lang);
            true
        } else {
            false
        }
    }

    pub fn advance_queue(&mut self, lang: &str) {
        if let Some(ref toast) = self.active_toast {
            let elapsed = toast.start_time.elapsed().as_secs_f32();
            if elapsed >= 4.8 {
                // Done displaying current toast
                self.active_toast = None;
            }
        }

        if self.active_toast.is_none() {
            if let Some(next_id) = self.queue.pop_front() {
                if let Some(def) = get_trophy_def(next_id) {
                    let title = if lang.starts_with("tr") {
                        def.name_tr.to_string()
                    } else {
                        def.name_en.to_string()
                    };

                    self.active_toast = Some(ActiveToast {
                        trophy_id: next_id,
                        grade: def.grade,
                        title,
                        start_time: Instant::now(),
                    });

                    // Play pop SFX for newly popped toast if more than one in queue
                    play_trophy_sound(&self.base_dir);
                }
            }
        }
    }

    pub fn trigger_test_notification(&mut self, lang: &str) {
        let test_id = (SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .subsec_millis()
            % 36) as u32;

        if let Some(def) = get_trophy_def(test_id) {
            let title = if lang.starts_with("tr") {
                def.name_tr.to_string()
            } else {
                def.name_en.to_string()
            };

            play_trophy_sound(&self.base_dir);

            self.active_toast = Some(ActiveToast {
                trophy_id: test_id,
                grade: def.grade,
                title,
                start_time: Instant::now(),
            });
        }
    }
}

// ---------------------------------------------------------------------------
// REAL-TIME TROPHY WATCHER SERVICE (FileSystem Watcher & Log Tailer)
// ---------------------------------------------------------------------------

pub struct TrophyWatcherService {
    stop_signal: Arc<AtomicBool>,
    current_lang: Arc<Mutex<String>>,
}

impl TrophyWatcherService {
    pub fn start(
        base_dir: PathBuf,
        shared_manager: Arc<Mutex<TrophyManager>>,
        lang: String,
    ) -> Self {
        let stop_signal = Arc::new(AtomicBool::new(false));
        let stop_clone = stop_signal.clone();
        let current_lang = Arc::new(Mutex::new(lang));
        let lang_clone = current_lang.clone();

        std::thread::Builder::new()
            .name("gow3-trophy-watcher".to_string())
            .spawn(move || {
                crate::log_info!("TrophyWatcherService background worker thread started.");

                let log_candidates = [
                    base_dir.join("gow3_log.txt"),
                    base_dir.join("out/gow3_log.txt"),
                    base_dir.join("gow3-linux/gow3_log.txt"),
                ];

                let mut last_log_size: u64 = 0;
                let mut last_file_idx = 0;

                while !stop_clone.load(Ordering::Relaxed) {
                    let active_lang = if let Ok(l) = lang_clone.lock() {
                        l.clone()
                    } else {
                        "tr".to_string()
                    };

                    // 1. Log Tailer: Inspect gow3_log.txt for new unlock lines
                    for (i, p) in log_candidates.iter().enumerate() {
                        if let Ok(meta) = fs::metadata(p) {
                            let curr_len = meta.len();
                            if i != last_file_idx {
                                last_file_idx = i;
                                last_log_size = 0;
                            }

                            if curr_len < last_log_size {
                                // Log file rotated or truncated
                                last_log_size = 0;
                            }

                            if curr_len > last_log_size {
                                if let Ok(mut f) = fs::File::open(p) {
                                    use std::io::{Read, Seek, SeekFrom};
                                    let _ = f.seek(SeekFrom::Start(last_log_size));
                                    let mut buf = String::new();
                                    if f.read_to_string(&mut buf).is_ok() {
                                        last_log_size = curr_len;
                                        parse_log_for_trophies(&buf, &shared_manager, &active_lang);
                                    }
                                }
                            }
                            break;
                        }
                    }

                    // 2. FileSystem Watcher: Inspect user/home XML files
                    let xml_candidates = [
                        base_dir.join("user/home/1/trophy/NPWR08268_00.xml"),
                        base_dir.join("user/trophy/NPWR08268_00.xml"),
                        base_dir.join("user/home/1/trophy/CUSA01715.xml"),
                        base_dir.join("user/trophy/CUSA01715.xml"),
                    ];
                    for xml_path in &xml_candidates {
                        if xml_path.is_file() {
                            if let Ok(content) = fs::read_to_string(xml_path) {
                                parse_xml_for_trophies(&content, &shared_manager, &active_lang);
                            }
                        }
                    }

                    // Advance active toast queue
                    if let Ok(mut mgr) = shared_manager.lock() {
                        mgr.advance_queue(&active_lang);
                    }

                    std::thread::sleep(Duration::from_millis(150));
                }

                crate::log_info!("TrophyWatcherService background worker stopped.");
            })
            .expect("Failed to spawn TrophyWatcherService thread");

        Self {
            stop_signal,
            current_lang,
        }
    }

    pub fn set_language(&self, new_lang: String) {
        if let Ok(mut l) = self.current_lang.lock() {
            *l = new_lang;
        }
    }

    pub fn stop(&self) {
        self.stop_signal.store(true, Ordering::Relaxed);
    }
}

impl Drop for TrophyWatcherService {
    fn drop(&mut self) {
        self.stop();
    }
}

fn parse_log_for_trophies(
    text: &str,
    shared_manager: &Arc<Mutex<TrophyManager>>,
    lang: &str,
) {
    for line in text.lines() {
        let line_lower = line.to_lowercase();
        if !line_lower.contains("trophy") {
            continue;
        }

        let mut found_id = None;

        let words: Vec<&str> = line_lower
            .split(|c: char| {
                c.is_whitespace()
                    || c == ':'
                    || c == '='
                    || c == ','
                    || c == '('
                    || c == ')'
                    || c == '['
                    || c == ']'
                    || c == '"'
                    || c == '\''
            })
            .filter(|s| !s.is_empty())
            .collect();

        for i in 0..words.len() {
            let w = words[i];
            if w == "trophy" || w == "unlocked" || w == "id" {
                if i + 1 < words.len() {
                    if let Ok(id) = words[i + 1].parse::<u32>() {
                        if id <= 35 {
                            found_id = Some(id);
                            break;
                        }
                    }
                }
                if i + 2 < words.len() && (words[i + 1] == "unlocked" || words[i + 1] == "id") {
                    if let Ok(id) = words[i + 2].parse::<u32>() {
                        if id <= 35 {
                            found_id = Some(id);
                            break;
                        }
                    }
                }
            }
        }

        if let Some(id) = found_id {
            if let Ok(mut mgr) = shared_manager.lock() {
                mgr.unlock_and_notify(id, lang);
            }
        }
    }
}

fn parse_xml_for_trophies(
    xml: &str,
    shared_manager: &Arc<Mutex<TrophyManager>>,
    lang: &str,
) {
    for line in xml.lines() {
        if line.contains("<trophy") && line.contains("unlockstate=\"true\"") {
            if let Some(id_str) = extract_attribute(line, "id") {
                if let Ok(id) = id_str.parse::<u32>() {
                    if id <= 35 {
                        if let Ok(mut mgr) = shared_manager.lock() {
                            if !mgr.db.is_unlocked(id) {
                                mgr.unlock_and_notify(id, lang);
                            }
                        }
                    }
                }
            }
        }
    }
}

// ---------------------------------------------------------------------------
// PS4 STYLE TOAST NOTIFICATION RENDERER
// ---------------------------------------------------------------------------

pub fn render_ps4_toast(
    ui: &mut egui::Ui,
    toast: &ActiveToast,
    lang: &str,
    is_standalone_viewport: bool,
) {
    let elapsed = toast.start_time.elapsed().as_secs_f32();
    if elapsed >= 4.8 {
        return;
    }

    // Slide-in (0.0..0.4s) -> Stay (0.4..4.4s) -> Fade-out (4.4..4.8s)
    let (offset_x, alpha) = if elapsed < 0.4 {
        let progress = (elapsed / 0.4).clamp(0.0, 1.0);
        let ease_out = 1.0 - (1.0 - progress).powi(3);
        ((1.0 - ease_out) * 380.0, ease_out)
    } else if elapsed <= 4.4 {
        (0.0, 1.0)
    } else {
        let fade_progress = ((elapsed - 4.4) / 0.4).clamp(0.0, 1.0);
        (0.0, 1.0 - fade_progress)
    };

    let toast_w = 360.0;
    let toast_h = 72.0;

    let rect = if is_standalone_viewport {
        Rect::from_min_size(Pos2::new(10.0 + offset_x, 8.0), Vec2::new(toast_w, toast_h))
    } else {
        let screen_rect = ui.max_rect();
        let min_x = screen_rect.max.x - toast_w - 24.0 + offset_x;
        let min_y = screen_rect.min.y + 24.0;
        Rect::from_min_size(Pos2::new(min_x, min_y), Vec2::new(toast_w, toast_h))
    };

    // 1. Subtle drop shadow
    let shadow_rect = rect.expand(2.5);
    ui.painter().rect_filled(
        shadow_rect,
        38.0,
        Color32::from_rgba_unmultiplied(0, 0, 0, (140.0 * alpha) as u8),
    );

    // 2. PS4 Iconic Pill background: rgba(18, 18, 18, 0.92)
    ui.painter().rect_filled(
        rect,
        36.0,
        Color32::from_rgba_unmultiplied(18, 18, 18, (235.0 * alpha) as u8),
    );

    // 3. Subtle metallic border
    let border_color = match toast.grade {
        TrophyGrade::Platinum => Color32::from_rgba_unmultiplied(160, 215, 255, (220.0 * alpha) as u8),
        TrophyGrade::Gold => Color32::from_rgba_unmultiplied(240, 200, 80, (220.0 * alpha) as u8),
        TrophyGrade::Silver => Color32::from_rgba_unmultiplied(200, 210, 220, (200.0 * alpha) as u8),
        TrophyGrade::Bronze => Color32::from_rgba_unmultiplied(210, 140, 90, (200.0 * alpha) as u8),
    };
    ui.painter().rect_stroke(rect, 36.0, Stroke::new(1.4, border_color), egui::StrokeKind::Inside);

    // 4. Draw Trophy Grade Icon (Left)
    let icon_rect = Rect::from_min_size(
        Pos2::new(rect.min.x + 12.0, rect.min.y + (toast_h - 48.0) * 0.5),
        Vec2::new(48.0, 48.0),
    );

    let icon_uri = match toast.grade {
        TrophyGrade::Platinum => "bytes://platinum.png",
        TrophyGrade::Gold => "bytes://gold.png",
        TrophyGrade::Silver => "bytes://silver.png",
        TrophyGrade::Bronze => "bytes://bronze.png",
    };

    ui.put(
        icon_rect,
        egui::Image::from_bytes(icon_uri, toast.grade.icon_bytes())
            .fit_to_exact_size(Vec2::new(48.0, 48.0))
            .tint(Color32::from_rgba_unmultiplied(255, 255, 255, (255.0 * alpha) as u8)),
    );

    let painter = ui.painter();

    // 5. Two-line typography (Right)
    let text_x = rect.min.x + 70.0;
    let header_y = rect.min.y + 16.0;
    let title_y = rect.min.y + 36.0;

    let header_text = if lang.starts_with("tr") {
        "KUPA KAZANILDI!"
    } else if lang.starts_with("de") {
        "TROPHÄE ERHALTEN!"
    } else if lang.starts_with("es") {
        "¡TROFEO CONSEGUIDO!"
    } else {
        "TROPHY EARNED!"
    };

    let header_color = match toast.grade {
        TrophyGrade::Platinum => Color32::from_rgba_unmultiplied(190, 230, 255, (255.0 * alpha) as u8),
        TrophyGrade::Gold => Color32::from_rgba_unmultiplied(255, 215, 80, (255.0 * alpha) as u8),
        _ => Color32::from_rgba_unmultiplied(220, 220, 220, (240.0 * alpha) as u8),
    };

    painter.text(
        Pos2::new(text_x, header_y),
        egui::Align2::LEFT_TOP,
        header_text,
        egui::FontId::proportional(12.0),
        header_color,
    );

    let display_title = if toast.title.chars().count() > 28 {
        format!("{}...", toast.title.chars().take(26).collect::<String>())
    } else {
        toast.title.clone()
    };

    painter.text(
        Pos2::new(text_x, title_y),
        egui::Align2::LEFT_TOP,
        display_title,
        egui::FontId::proportional(15.5),
        Color32::from_rgba_unmultiplied(255, 255, 255, (255.0 * alpha) as u8),
    );

    // Request continuous repaint while animation is active
    ui.ctx().request_repaint();
}

// ---------------------------------------------------------------------------
// TROPHY DASHBOARD & SHOWCASE UI
// ---------------------------------------------------------------------------

pub fn render_trophy_dashboard(
    ui: &mut egui::Ui,
    manager: &mut TrophyManager,
    lang: &str,
    on_back_clicked: &mut bool,
) {
    let is_tr = lang.starts_with("tr");

    ui.vertical(|ui| {
        // --- 1. TOP BAR: BACK BUTTON & TITLE ---
        ui.horizontal(|ui| {
            let back_label = if is_tr { "◀  Geri Dön" } else { "◀  Back" };
            if ui.add(egui::Button::new(egui::RichText::new(back_label).size(14.0).color(Color32::WHITE))
                .min_size(Vec2::new(110.0, 32.0))).clicked() {
                *on_back_clicked = true;
            }

            ui.add_space(16.0);

            let header_title = if is_tr {
                "🏆  GOD OF WAR® III — KUPA MERKEZİ"
            } else {
                "🏆  GOD OF WAR® III — TROPHIES SHOWCASE"
            };
            ui.heading(egui::RichText::new(header_title).size(20.0).color(Color32::from_rgb(255, 215, 80)));

            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                let test_btn_text = if is_tr { "🔔 Test Bildirimi" } else { "🔔 Test Notification" };
                if ui.add(egui::Button::new(egui::RichText::new(test_btn_text).size(13.0).color(Color32::from_rgb(220, 230, 255)))
                    .min_size(Vec2::new(140.0, 30.0))).clicked() {
                    manager.trigger_test_notification(lang);
                }
            });
        });

        ui.add_space(12.0);

        // --- 2. HEADER & PROGRESS ANALYTICS ---
        let total_count = manager.db.total_count();
        let unlocked_count = manager.db.unlocked_count();
        let percentage = manager.db.percentage();
        let is_mastered = manager.db.is_mastered();

        egui::Frame::new()
            .fill(Color32::from_rgba_unmultiplied(22, 23, 30, 230))
            .stroke(Stroke::new(1.0, Color32::from_rgba_unmultiplied(60, 65, 80, 200)))
            .corner_radius(CornerRadius::same(10))
            .inner_margin(egui::Margin::same(14))
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    // Left: Progress Analytics
                    ui.vertical(|ui| {
                        ui.horizontal(|ui| {
                            let prog_title = if is_tr { "GENEL KUPA İLERLEMESİ" } else { "OVERALL TROPHY PROGRESS" };
                            ui.label(egui::RichText::new(prog_title).size(12.0).strong().color(Color32::from_rgb(180, 190, 210)));

                            ui.add_space(8.0);
                            let perc_text = format!("{:.0}%  ({} / {})", percentage, unlocked_count, total_count);
                            ui.label(egui::RichText::new(perc_text).size(14.0).strong().color(Color32::from_rgb(255, 215, 80)));

                            // 100% Mastery Badge
                            if is_mastered {
                                ui.add_space(12.0);
                                let mastery_text = if is_tr {
                                    "👑 %100 PLATİN USTALIK MÜHRÜ"
                                } else {
                                    "👑 100% PLATINUM MASTERY BADGE"
                                };
                                egui::Frame::new()
                                    .fill(Color32::from_rgba_unmultiplied(40, 80, 140, 220))
                                    .stroke(Stroke::new(1.5, Color32::from_rgb(180, 225, 255)))
                                    .corner_radius(CornerRadius::same(14))
                                    .inner_margin(egui::Margin::symmetric(10, 4))
                                    .show(ui, |ui| {
                                        ui.label(egui::RichText::new(mastery_text).size(12.5).strong().color(Color32::from_rgb(220, 240, 255)));
                                    });
                            }
                        });

                        ui.add_space(6.0);

                        // Progress Bar
                        let progress_fraction = (unlocked_count as f32 / total_count as f32).clamp(0.0, 1.0);
                        let bar_rect = ui.allocate_space(Vec2::new(ui.available_width().min(600.0), 12.0)).1;
                        ui.painter().rect_filled(bar_rect, 6.0, Color32::from_rgb(30, 32, 42));
                        if progress_fraction > 0.0 {
                            let filled_w = bar_rect.width() * progress_fraction;
                            let filled_rect = Rect::from_min_size(bar_rect.min, Vec2::new(filled_w, bar_rect.height()));
                            let bar_color = if is_mastered {
                                Color32::from_rgb(160, 220, 255)
                            } else {
                                Color32::from_rgb(230, 180, 50)
                            };
                            ui.painter().rect_filled(filled_rect, 6.0, bar_color);
                        }
                    });
                });

                ui.add_space(10.0);
                ui.separator();
                ui.add_space(8.0);

                // Kupa Dağılım Sayacı (Trophy Breakdown Count)
                let plat_u = manager.db.unlocked_by_grade(TrophyGrade::Platinum);
                let plat_t = manager.db.total_by_grade(TrophyGrade::Platinum);
                let gold_u = manager.db.unlocked_by_grade(TrophyGrade::Gold);
                let gold_t = manager.db.total_by_grade(TrophyGrade::Gold);
                let silv_u = manager.db.unlocked_by_grade(TrophyGrade::Silver);
                let silv_t = manager.db.total_by_grade(TrophyGrade::Silver);
                let bron_u = manager.db.unlocked_by_grade(TrophyGrade::Bronze);
                let bron_t = manager.db.total_by_grade(TrophyGrade::Bronze);

                ui.horizontal_wrapped(|ui| {
                    render_stat_pill(ui, "bytes://platinum.png", ICON_PLATINUM, TrophyGrade::Platinum.name_tr(), plat_u, plat_t, TrophyGrade::Platinum.badge_color());
                    ui.add_space(14.0);
                    render_stat_pill(ui, "bytes://gold.png", ICON_GOLD, TrophyGrade::Gold.name_tr(), gold_u, gold_t, TrophyGrade::Gold.badge_color());
                    ui.add_space(14.0);
                    render_stat_pill(ui, "bytes://silver.png", ICON_SILVER, TrophyGrade::Silver.name_tr(), silv_u, silv_t, TrophyGrade::Silver.badge_color());
                    ui.add_space(14.0);
                    render_stat_pill(ui, "bytes://bronze.png", ICON_BRONZE, TrophyGrade::Bronze.name_tr(), bron_u, bron_t, TrophyGrade::Bronze.badge_color());
                });
            });

        ui.add_space(10.0);

        // --- 3. FILTER & SEARCH CONTROLS ---
        ui.horizontal(|ui| {
            // Filter buttons
            let filters = [
                (TrophyFilter::All, if is_tr { "Tümü (All)" } else { "All" }),
                (TrophyFilter::Earned, if is_tr { "Kazanılanlar (Earned)" } else { "Earned" }),
                (TrophyFilter::Locked, if is_tr { "Kilitliler (Locked)" } else { "Locked" }),
                (TrophyFilter::Secret, if is_tr { "Gizli Kupalar (Secret)" } else { "Secret" }),
            ];

            for (f, label) in filters {
                let is_selected = manager.filter == f;
                let btn = egui::Button::new(
                    egui::RichText::new(label)
                        .size(13.0)
                        .strong()
                        .color(if is_selected { Color32::from_rgb(255, 215, 80) } else { Color32::from_rgb(180, 185, 200) }),
                )
                .fill(if is_selected { Color32::from_rgb(45, 48, 65) } else { Color32::from_rgb(25, 26, 35) })
                .stroke(Stroke::new(1.0, if is_selected { Color32::from_rgb(230, 180, 50) } else { Color32::from_rgb(50, 52, 65) }))
                .min_size(Vec2::new(110.0, 28.0));

                if ui.add(btn).clicked() {
                    manager.filter = f;
                }
                ui.add_space(6.0);
            }

            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                let search_hint = if is_tr { "🔍 Kupa Ara..." } else { "🔍 Search trophies..." };
                ui.add(egui::TextEdit::singleline(&mut manager.search_query)
                    .hint_text(search_hint)
                    .desired_width(200.0));
            });
        });

        ui.add_space(10.0);

        // --- 4. TROPHY CARDS LIST ---
        egui::ScrollArea::vertical()
            .auto_shrink([false, false])
            .show(ui, |ui| {
                ui.vertical(|ui| {
                    let search = manager.search_query.trim().to_lowercase();

                    for def in ALL_TROPHIES.iter() {
                        let state = manager.db.get_state(def.id);
                        let is_unlocked = state.unlocked;

                        // Filter check
                        match manager.filter {
                            TrophyFilter::Earned if !is_unlocked => continue,
                            TrophyFilter::Locked if is_unlocked => continue,
                            TrophyFilter::Secret if !def.hidden => continue,
                            _ => {}
                        }

                        let name = if is_tr { def.name_tr } else { def.name_en };
                        let desc = if is_tr { def.desc_tr } else { def.desc_en };

                        // Search check
                        if !search.is_empty() {
                            let match_name = name.to_lowercase().contains(&search);
                            let match_desc = desc.to_lowercase().contains(&search);
                            if !match_name && !match_desc {
                                continue;
                            }
                        }

                        render_trophy_card(ui, def, &state, is_tr, &mut manager.revealed_secrets);
                        ui.add_space(8.0);
                    }
                });
            });
    });
}

fn render_stat_pill(
    ui: &mut egui::Ui,
    icon_uri: &'static str,
    icon_bytes: &'static [u8],
    grade_name: &str,
    unlocked: usize,
    total: usize,
    color: Color32,
) {
    egui::Frame::new()
        .fill(Color32::from_rgb(28, 30, 40))
        .stroke(Stroke::new(1.0, Color32::from_rgba_unmultiplied(70, 75, 95, 180)))
        .corner_radius(CornerRadius::same(16))
        .inner_margin(egui::Margin::symmetric(10, 4))
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.add(
                    egui::Image::from_bytes(icon_uri, icon_bytes)
                        .fit_to_exact_size(Vec2::new(22.0, 22.0)),
                );
                ui.add_space(4.0);
                ui.label(egui::RichText::new(grade_name).size(12.5).strong().color(color));
                ui.add_space(6.0);
                let count_str = format!("{}/{}", unlocked, total);
                ui.label(egui::RichText::new(count_str).size(13.0).strong().color(Color32::WHITE));
            });
        });
}

fn render_trophy_card(
    ui: &mut egui::Ui,
    def: &TrophyDefinition,
    state: &TrophyItemState,
    is_tr: bool,
    revealed_secrets: &mut HashSet<u32>,
) {
    let is_unlocked = state.unlocked;
    let is_revealed = revealed_secrets.contains(&def.id);
    let is_secret_hidden = def.hidden && !is_unlocked && !is_revealed;

    let card_bg = if is_unlocked {
        Color32::from_rgba_unmultiplied(26, 28, 38, 240)
    } else {
        Color32::from_rgba_unmultiplied(18, 19, 24, 210)
    };

    let border_color = if is_unlocked {
        match def.grade {
            TrophyGrade::Platinum => Color32::from_rgba_unmultiplied(120, 180, 240, 160),
            TrophyGrade::Gold => Color32::from_rgba_unmultiplied(220, 170, 50, 160),
            TrophyGrade::Silver => Color32::from_rgba_unmultiplied(170, 180, 200, 140),
            TrophyGrade::Bronze => Color32::from_rgba_unmultiplied(180, 120, 75, 140),
        }
    } else {
        Color32::from_rgba_unmultiplied(50, 52, 65, 140)
    };

    egui::Frame::new()
        .fill(card_bg)
        .stroke(Stroke::new(1.0, border_color))
        .corner_radius(CornerRadius::same(8))
        .inner_margin(egui::Margin::same(10))
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                // 1. Trophy Image
                let icon_uri = format!("bytes://TROP{:03}.PNG", def.id);
                let img_tint = if is_unlocked {
                    Color32::WHITE
                } else {
                    Color32::from_rgba_unmultiplied(130, 130, 140, 140)
                };

                ui.add(
                    egui::Image::from_bytes(icon_uri, def.icon_bytes)
                        .fit_to_exact_size(Vec2::new(56.0, 56.0))
                        .tint(img_tint),
                );

                ui.add_space(12.0);

                // 2. Middle Content: Grade Pill, Name, Description
                ui.vertical(|ui| {
                    ui.horizontal(|ui| {
                        // Grade Pill
                        let grade_text = if is_tr { def.grade.name_tr() } else { def.grade.name_en() };
                        egui::Frame::new()
                            .fill(Color32::from_rgb(32, 34, 45))
                            .stroke(Stroke::new(1.0, def.grade.badge_color()))
                            .corner_radius(CornerRadius::same(4))
                            .inner_margin(egui::Margin::symmetric(6, 2))
                            .show(ui, |ui| {
                                ui.label(egui::RichText::new(grade_text).size(11.0).strong().color(def.grade.badge_color()));
                            });

                        ui.add_space(8.0);

                        // Trophy Title
                        if is_secret_hidden {
                            let hidden_title = if is_tr { "🔒  Gizli Kupa" } else { "🔒  Secret Trophy" };
                            ui.label(egui::RichText::new(hidden_title).size(15.0).italics().color(Color32::from_rgb(160, 165, 180)));
                        } else {
                            let name = if is_tr { def.name_tr } else { def.name_en };
                            let name_color = if is_unlocked { Color32::WHITE } else { Color32::from_rgb(180, 185, 200) };
                            ui.label(egui::RichText::new(name).size(15.5).strong().color(name_color));
                        }
                    });

                    ui.add_space(4.0);

                    // Description
                    if is_secret_hidden {
                        let hidden_desc = if is_tr {
                            "Gizli bir kupa. Oyunda ilerledikçe ayrıntıları ortaya çıkar."
                        } else {
                            "This is a secret trophy. Progress further in the game to reveal it."
                        };
                        ui.label(egui::RichText::new(hidden_desc).size(12.5).italics().color(Color32::from_rgb(120, 125, 140)));
                    } else {
                        let desc = if is_tr { def.desc_tr } else { def.desc_en };
                        let desc_color = if is_unlocked { Color32::from_rgb(205, 210, 220) } else { Color32::from_rgb(130, 135, 145) };
                        ui.label(egui::RichText::new(desc).size(13.0).color(desc_color));
                    }
                });

                // 3. Right Column: Status / Toggle / Timestamp
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if is_unlocked {
                        ui.vertical(|ui| {
                            let unlocked_title = if is_tr { "✓  KAZANILDI" } else { "✓  UNLOCKED" };
                            ui.label(egui::RichText::new(unlocked_title).size(12.5).strong().color(Color32::from_rgb(120, 220, 120)));

                            if let Some(ref date_str) = state.unlocked_at_formatted {
                                ui.label(egui::RichText::new(date_str).size(11.5).color(Color32::from_rgb(220, 200, 120)));
                            }
                        });
                    } else if def.hidden {
                        let toggle_text = if is_revealed {
                            if is_tr { "Gizle" } else { "Hide" }
                        } else {
                            if is_tr { "👁️ Detayları Göster" } else { "👁️ Show Details" }
                        };
                        if ui.add(egui::Button::new(egui::RichText::new(toggle_text).size(12.0).color(Color32::from_rgb(180, 200, 240)))
                            .min_size(Vec2::new(130.0, 26.0))).clicked() {
                            if is_revealed {
                                revealed_secrets.remove(&def.id);
                            } else {
                                revealed_secrets.insert(def.id);
                            }
                        }
                    } else {
                        let locked_text = if is_tr { "🔒  Kilitli" } else { "🔒  Locked" };
                        ui.label(egui::RichText::new(locked_text).size(13.0).color(Color32::from_rgb(120, 125, 140)));
                    }
                });
            });
        });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_all_trophies_count_and_grades() {
        assert_eq!(ALL_TROPHIES.len(), 36);

        let mut plat = 0;
        let mut gold = 0;
        let mut silver = 0;
        let mut bronze = 0;

        for (idx, t) in ALL_TROPHIES.iter().enumerate() {
            assert_eq!(t.id, idx as u32);
            assert!(!t.name_en.is_empty());
            assert!(!t.name_tr.is_empty());
            assert!(!t.desc_en.is_empty());
            assert!(!t.desc_tr.is_empty());
            assert!(!t.icon_bytes.is_empty());

            match t.grade {
                TrophyGrade::Platinum => plat += 1,
                TrophyGrade::Gold => gold += 1,
                TrophyGrade::Silver => silver += 1,
                TrophyGrade::Bronze => bronze += 1,
            }
        }

        assert_eq!(plat, 1);
        assert_eq!(gold, 5);
        assert_eq!(silver, 10);
        assert_eq!(bronze, 20);
        assert_eq!(ALL_TROPHIES[0].grade, TrophyGrade::Platinum);
    }

    #[test]
    fn test_trophy_database_persistence() {
        let tmp_dir = std::env::temp_dir().join(format!("gow3_trophy_test_{}", std::process::id()));
        let _ = fs::create_dir_all(&tmp_dir);

        let mut db = TrophyStateDatabase::load(&tmp_dir);
        assert_eq!(db.total_count(), 36);
        assert_eq!(db.unlocked_count(), 0);
        assert_eq!(db.percentage(), 0.0);
        assert!(!db.is_mastered());

        let now = 1730000000;
        assert!(db.unlock(1, now));
        assert!(db.unlock(5, now + 100));
        assert_eq!(db.unlocked_count(), 2);
        assert!((db.percentage() - (2.0 / 36.0 * 100.0)).abs() < 0.01);

        db.save(&tmp_dir);

        let reloaded = TrophyStateDatabase::load(&tmp_dir);
        assert_eq!(reloaded.unlocked_count(), 2);
        assert!(reloaded.is_unlocked(1));
        assert!(reloaded.is_unlocked(5));
        assert!(!reloaded.is_unlocked(0));

        let state1 = reloaded.get_state(1);
        assert!(state1.unlocked);
        assert!(state1.unlocked_at_formatted.is_some());

        let _ = fs::remove_dir_all(&tmp_dir);
    }

    #[test]
    fn test_platinum_auto_unlock() {
        let tmp_dir = std::env::temp_dir().join(format!("gow3_plat_test_{}", std::process::id()));
        let _ = fs::create_dir_all(&tmp_dir);

        let mut mgr = TrophyManager::new(&tmp_dir);
        assert!(!mgr.db.is_unlocked(0));

        for i in 1..=35 {
            mgr.unlock_and_notify(i, "tr");
        }

        assert_eq!(mgr.db.unlocked_count(), 36);
        assert!(mgr.db.is_unlocked(0));
        assert!(mgr.db.is_mastered());
        assert_eq!(mgr.db.percentage(), 100.0);

        let _ = fs::remove_dir_all(&tmp_dir);
    }

    #[test]
    fn test_parse_log_for_trophies() {
        let tmp_dir = std::env::temp_dir().join(format!("gow3_log_test_{}", std::process::id()));
        let _ = fs::create_dir_all(&tmp_dir);

        let mgr = Arc::new(Mutex::new(TrophyManager::new(&tmp_dir)));

        let log_sample = r#"
[00:01:23] [Trophy] Runtime: trophy 1 unlocked
[00:02:45] sceNpTrophyUnlockTrophy called for id 7
Random log line without trophy
Trophy unlocked: 12
"#;

        parse_log_for_trophies(log_sample, &mgr, "tr");

        let m = mgr.lock().unwrap();
        assert!(m.db.is_unlocked(1));
        assert!(m.db.is_unlocked(7));
        assert!(m.db.is_unlocked(12));
        assert!(!m.db.is_unlocked(2));

        let _ = fs::remove_dir_all(&tmp_dir);
    }

    #[test]
    fn test_timestamp_formatting() {
        let ts = 1730000000;
        let formatted = format_timestamp(ts);
        assert!(formatted.contains('.'));
        assert!(formatted.contains(':'));
        assert!(formatted.contains(" - "));
    }
}


