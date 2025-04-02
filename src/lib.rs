#![no_std]
#![warn(
    clippy::complexity,
    clippy::correctness,
    clippy::perf,
    clippy::style,
    clippy::undocumented_unsafe_blocks,
    rust_2018_idioms
)]

use asr::{
    file_format::pe,
    future::{next_tick, retry},
    settings::{gui::Title, Gui},
    signature::Signature,
    time::Duration,
    timer::{self, TimerState},
    watcher::Watcher,
    Address, Process,
};

asr::async_main!(stable);
asr::panic_handler!();

const PROCESS_NAMES: &[&str] = &["Croc64.exe"];

async fn main() {
    let mut settings = Settings::register();

    loop {
        // Hook to the target process
        let (process_name, process) = retry(|| {
            PROCESS_NAMES
                .iter()
                .find_map(|&name| Some((name, Process::attach(name)?)))
        })
        .await;

        process
            .until_closes(async {
                // Once the target has been found and attached to, set up some default watchers
                let mut watchers = Watchers::default();

                // Perform memory scanning to look for the addresses we need
                let addresses = Memory::init(&process, process_name).await;

                loop {
                    // Splitting logic. Adapted from OG LiveSplit:
                    // Order of execution
                    // 1. update() will always be run first. There are no conditions on the execution of this action.
                    // 2. If the timer is currently either running or paused, then the isLoading, gameTime, and reset actions will be run.
                    // 3. If reset does not return true, then the split action will be run.
                    // 4. If the timer is currently not running (and not paused), then the start action will be run.
                    settings.update();
                    update_loop(&process, &addresses, &mut watchers);

                    if [TimerState::Running, TimerState::Paused].contains(&timer::state()) {
                        match is_loading(&watchers, &settings) {
                            Some(true) => timer::pause_game_time(),
                            Some(false) => timer::resume_game_time(),
                            _ => (),
                        }

                        match game_time(&watchers, &settings, &addresses) {
                            Some(x) => timer::set_game_time(x),
                            _ => (),
                        }

                        match reset(&watchers, &settings) {
                            true => timer::reset(),
                            _ => match split(&watchers, &settings) {
                                true => timer::split(),
                                _ => (),
                            },
                        }
                    }

                    if timer::state().eq(&TimerState::NotRunning) && start(&watchers, &settings) {
                        timer::start();
                        timer::pause_game_time();

                        match is_loading(&watchers, &settings) {
                            Some(true) => timer::pause_game_time(),
                            Some(false) => timer::resume_game_time(),
                            _ => (),
                        }
                    }

                    next_tick().await;
                }
            })
            .await;
    }
}

#[derive(Gui)]
struct Settings {
    /// General settings
    _general: Title,
    /// Enable auto start
    #[default = true]
    start: bool,
    /// Level splitting
    _level: Title,
    /// 1-1 - And So The Adventure Begins
    #[default = true]
    level_1_1: bool,
    /// 1-2 - Underground Overground
    #[default = true]
    level_1_2: bool,
    /// 1-3 - Shoutin Lava Lava Lava
    #[default = true]
    level_1_3: bool,
    /// 1-B1 - Lair of the Feeble
    #[default = true]
    level_1_b1: bool,
    /// 1-S1 - The Curvy Caverns
    #[default = true]
    level_1_s1: bool,
    /// 1-4 - The Tumbling Dantini
    #[default = true]
    level_1_4: bool,
    /// 1-5 - Cave Fear
    #[default = true]
    level_1_5: bool,
    /// 1-6 - Darkness Descends
    #[default = true]
    level_1_6: bool,
    /// 1-B2 - Fight Night with Flibby
    #[default = true]
    level_1_b2: bool,
    /// 1-S2 - The Twisty Tunnels
    #[default = true]
    level_1_s2: bool,
    /// 2-1 - The Ice of Life
    #[default = true]
    level_2_1: bool,
    /// 2-2 - Be Wheely Careful
    #[default = true]
    level_2_2: bool,
    /// 2-3 - Riot Brrrrr
    #[default = true]
    level_2_3: bool,
    /// 2-B1 - Chumly's Snow Den
    #[default = true]
    level_2_b1: bool,
    /// 2-S1 - Clouds of Ice
    #[default = true]
    level_2_s1: bool,
    /// 2-4 - I Snow Him So Well
    #[default = true]
    level_2_4: bool,
    /// 2-5 - Say No Snow
    #[default = true]
    level_2_5: bool,
    /// 2-6 - Licence to Chill
    #[default = true]
    level_2_6: bool,
    /// 2-B2 - Demon Itsy's Ice Palace
    #[default = true]
    level_2_b2: bool,
    /// 1-S2 - Ice Bridge to Eternity
    #[default = true]
    level_2_s2: bool,
    /// 3-1 - Lights, Camel, Action!
    #[default = true]
    level_3_1: bool,
    /// 3-2 - Mud Pit Mania
    #[default = true]
    level_3_2: bool,
    /// 3-3 - Goin' Underground
    #[default = true]
    level_3_3: bool,
    /// 3-B1 - The Deadly Tank of Neptuna
    #[default = true]
    level_3_b1: bool,
    /// 3-S1 - Arabian Heights
    #[default = true]
    level_3_s1: bool,
    /// 3-4 - Sand and Freedom
    #[default = true]
    level_3_4: bool,
    /// 3-5 - Leap of Faith
    #[default = true]
    level_3_5: bool,
    /// 3-6 - Life's a Beach
    #[default = true]
    level_3_6: bool,
    /// 3-B2 - Cactus Jack's Ranch
    #[default = true]
    level_3_b2: bool,
    /// 3-S2 - Defeato Burrito
    #[default = true]
    level_3_s2: bool,
    /// 4-1 - The Tower of Power
    #[default = true]
    level_4_1: bool,
    /// 4-2 - Hassle in the Castle
    #[default = true]
    level_4_2: bool,
    /// 4-3 - Dungeon of Defright
    #[default = true]
    level_4_3: bool,
    /// 4-B1 - Fosley's Freaky Donut
    #[default = true]
    level_4_b1: bool,
    /// 4-S1 - Smash and See
    #[default = true]
    level_4_s1: bool,
    /// 4-4 - Ballistic Meg's Fairway
    #[default = true]
    level_4_4: bool,
    /// 4-5 - Swipe Swiftly's Wicked Ride
    #[default = true]
    level_4_5: bool,
    /// 4-6 - Panic at Platform Pete's Lair
    #[default = true]
    level_4_6: bool,
    /// 4-B2 - Baron Dante's Funky Inferno
    #[default = true]
    level_4_b2: bool,
    /// 4-S2 - Jailhouse Croc
    #[default = true]
    level_4_s2: bool,
    /// 5-1 - And So The Adventure Returns
    #[default = true]
    level_5_1: bool,
    /// 5-2 - Diet Brrrrrrr
    #[default = true]
    level_5_2: bool,
    /// 5-3 - Trial on the Nile
    #[default = true]
    level_5_3: bool,
    /// 5-4 - Crox Interactive
    #[default = true]
    level_5_4: bool,
    /// 5-B1 - Secret Sentinel
    #[default = true]
    level_5_b1: bool,
}

struct Memory {
    level_id: Address,
    game_status: Address,
    level_completion_flag: Address,
}

impl Memory {
    async fn init(process: &Process, main_module_name: &str) -> Self {
        let main_module_base = retry(|| process.get_module_address(main_module_name)).await;
        let main_module_size = retry(|| pe::read_size_of_image(process, main_module_base)).await;
        let main_module = (main_module_base, main_module_size as u64);

        const LEVEL_ID: Signature<13> = Signature::new("0F 85 ?? ?? ?? ?? 8B 05 ?? ?? ?? ?? B9");
        let level_id = retry(|| {
            LEVEL_ID
                .scan_process_range(process, main_module)
                .map(|val| val + 8)
                .and_then(|addr: Address| Some(addr + 0x4 + process.read::<i32>(addr).ok()?))
        })
        .await;

        const GAME_STATUS: Signature<13> = Signature::new("89 05 ?? ?? ?? ?? 83 0D ?? ?? ?? ?? 01");
        let game_status = retry(|| {
            GAME_STATUS
                .scan_process_range(process, main_module)
                .map(|val| val + 2)
                .and_then(|addr: Address| Some(addr + 0x4 + process.read::<i32>(addr).ok()?))
        })
        .await;

        const LEVEL_COMPLETE_SCREEN: Signature<12> =
            Signature::new("48 83 EC ?? C6 05 ?? ?? ?? ?? 01 C6");
        let level_completion_flag: Address = retry(|| {
            LEVEL_COMPLETE_SCREEN
                .scan_process_range(process, main_module)
                .map(|val| val + 6)
                .and_then(|addr: Address| Some(addr + 0x5 + process.read::<i32>(addr).ok()?))
        })
        .await
            + 1;

        Self {
            level_id,
            game_status,
            level_completion_flag,
        }
    }
}

#[derive(Default)]
struct Watchers {
    level: Watcher<Level>,
    level_complete_flag: Watcher<bool>,
    game_status: Watcher<GameStatus>,
}

#[allow(non_camel_case_types)]
#[derive(Copy, Clone, Debug, Hash, Eq, PartialEq)]
enum Level {
    L1_1,
    L1_2,
    L1_3,
    L1_B1,
    L1_S1,
    L1_4,
    L1_5,
    L1_6,
    L1_B2,
    L1_S2,
    L2_1,
    L2_2,
    L2_3,
    L2_B1,
    L2_S1,
    L2_4,
    L2_5,
    L2_6,
    L2_B2,
    L2_S2,
    L3_1,
    L3_2,
    L3_3,
    L3_B1,
    L3_S1,
    L3_4,
    L3_5,
    L3_6,
    L3_B2,
    L3_S2,
    L4_1,
    L4_2,
    L4_3,
    L4_B1,
    L4_S1,
    L4_4,
    L4_5,
    L4_6,
    L4_B2,
    L4_S2,
    L5_1,
    L5_2,
    L5_3,
    L5_4,
    L5_B1,
}

#[derive(Copy, Clone, Debug, Hash, Eq, PartialEq)]
enum GameStatus {
    Intro,
    DemoMode,
    MainMenu,
    WorldMap,
    InGame,
    Unknown,
}

fn update_loop(process: &Process, memory: &Memory, watchers: &mut Watchers) {
    watchers
        .game_status
        .update_infallible(match process.read::<u32>(memory.game_status) {
            Ok(2) => GameStatus::DemoMode,
            Ok(3) => GameStatus::MainMenu,
            Ok(5) => GameStatus::InGame,
            Ok(8) => GameStatus::WorldMap,
            Ok(12) => GameStatus::Intro,
            _ => GameStatus::Unknown,
        });

    watchers.level_complete_flag.update_infallible(
        process
            .read::<u8>(memory.level_completion_flag)
            .is_ok_and(|val| val != 0),
    );

    watchers
        .level
        .update_infallible(match process.read::<u32>(memory.level_id) {
            Ok(10) => Level::L1_1,
            Ok(11) => Level::L1_2,
            Ok(12) => Level::L1_3,
            Ok(13) => Level::L1_B1,
            Ok(14) => Level::L1_4,
            Ok(15) => Level::L1_5,
            Ok(16) => Level::L1_6,
            Ok(17) => Level::L1_B2,
            Ok(18) => Level::L1_S1,
            Ok(19) => Level::L1_S2,
            Ok(20) => Level::L2_1,
            Ok(21) => Level::L2_2,
            Ok(22) => Level::L2_3,
            Ok(23) => Level::L2_B1,
            Ok(24) => Level::L2_4,
            Ok(25) => Level::L2_5,
            Ok(26) => Level::L2_6,
            Ok(27) => Level::L2_B2,
            Ok(28) => Level::L2_S1,
            Ok(29) => Level::L2_S2,
            Ok(30) => Level::L3_1,
            Ok(31) => Level::L3_2,
            Ok(32) => Level::L3_3,
            Ok(33) => Level::L3_B1,
            Ok(34) => Level::L3_4,
            Ok(35) => Level::L3_5,
            Ok(36) => Level::L3_6,
            Ok(37) => Level::L3_B2,
            Ok(38) => Level::L3_S1,
            Ok(39) => Level::L3_S2,
            Ok(40) => Level::L4_1,
            Ok(41) => Level::L4_2,
            Ok(42) => Level::L4_3,
            Ok(43) => Level::L4_B1,
            Ok(44) => Level::L4_4,
            Ok(45) => Level::L4_5,
            Ok(46) => Level::L4_6,
            Ok(47) => Level::L4_B2,
            Ok(48) => Level::L4_S1,
            Ok(49) => Level::L4_S2,
            Ok(50) => Level::L5_1,
            Ok(51) => Level::L5_2,
            Ok(52) => Level::L5_3,
            Ok(53) => Level::L5_4,
            Ok(54) => Level::L5_B1,
            _ => Level::L1_1,
        });
}

fn start(watchers: &Watchers, settings: &Settings) -> bool {
    if !settings.start {
        return false;
    }

    watchers
        .game_status
        .pair
        .is_some_and(|val| val.changed_from_to(&GameStatus::MainMenu, &GameStatus::WorldMap))
        && watchers
            .level
            .pair
            .is_some_and(|val| val.current.eq(&Level::L1_1))
}

fn is_loading(_watchers: &Watchers, _settings: &Settings) -> Option<bool> {
    None
}

fn split(watchers: &Watchers, settings: &Settings) -> bool {
    watchers
        .game_status
        .pair
        .is_some_and(|val| val.current.eq(&GameStatus::InGame))
        && watchers
            .level_complete_flag
            .pair
            .is_some_and(|val| val.changed_from_to(&false, &true))
        && match watchers.level.pair.map(|val| val.old) {
            Some(Level::L1_1) => settings.level_1_1,
            Some(Level::L1_2) => settings.level_1_2,
            Some(Level::L1_3) => settings.level_1_3,
            Some(Level::L1_4) => settings.level_1_4,
            Some(Level::L1_5) => settings.level_1_5,
            Some(Level::L1_6) => settings.level_1_6,
            Some(Level::L1_B1) => settings.level_1_b1,
            Some(Level::L1_B2) => settings.level_1_b2,
            Some(Level::L1_S1) => settings.level_1_s1,
            Some(Level::L1_S2) => settings.level_1_s2,
            Some(Level::L2_1) => settings.level_2_1,
            Some(Level::L2_2) => settings.level_2_2,
            Some(Level::L2_3) => settings.level_2_3,
            Some(Level::L2_4) => settings.level_2_4,
            Some(Level::L2_5) => settings.level_2_5,
            Some(Level::L2_6) => settings.level_2_6,
            Some(Level::L2_B1) => settings.level_2_b1,
            Some(Level::L2_B2) => settings.level_2_b2,
            Some(Level::L2_S1) => settings.level_2_s1,
            Some(Level::L2_S2) => settings.level_2_s2,
            Some(Level::L3_1) => settings.level_3_1,
            Some(Level::L3_2) => settings.level_3_2,
            Some(Level::L3_3) => settings.level_3_3,
            Some(Level::L3_4) => settings.level_3_4,
            Some(Level::L3_5) => settings.level_3_5,
            Some(Level::L3_6) => settings.level_3_6,
            Some(Level::L3_B1) => settings.level_3_b1,
            Some(Level::L3_B2) => settings.level_3_b2,
            Some(Level::L3_S1) => settings.level_3_s1,
            Some(Level::L3_S2) => settings.level_3_s2,
            Some(Level::L4_1) => settings.level_4_1,
            Some(Level::L4_2) => settings.level_4_2,
            Some(Level::L4_3) => settings.level_4_3,
            Some(Level::L4_4) => settings.level_4_4,
            Some(Level::L4_5) => settings.level_4_5,
            Some(Level::L4_6) => settings.level_4_6,
            Some(Level::L4_B1) => settings.level_4_b1,
            Some(Level::L4_B2) => settings.level_4_b2,
            Some(Level::L4_S1) => settings.level_4_s1,
            Some(Level::L4_S2) => settings.level_4_s2,
            Some(Level::L5_1) => settings.level_5_1,
            Some(Level::L5_2) => settings.level_5_2,
            Some(Level::L5_3) => settings.level_5_3,
            Some(Level::L5_4) => settings.level_5_4,
            Some(Level::L5_B1) => settings.level_5_b1,
            _ => false,
        }
}

fn game_time(_watchers: &Watchers, _settings: &Settings, _addresses: &Memory) -> Option<Duration> {
    None
}

fn reset(_watchers: &Watchers, _settings: &Settings) -> bool {
    false
}
