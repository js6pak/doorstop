use std::{
    env,
    env::Args,
    iter::Peekable,
    path,
    path::{Path, PathBuf},
};

use ini::{Ini, Properties};

#[derive(Debug)]
#[allow(clippy::struct_excessive_bools)]
pub(crate) struct Config {
    pub enabled: bool,
    pub redirect_output_log: bool,
    pub ignore_disabled_env: bool,
    pub target_assembly: Option<PathBuf>,
    pub boot_config_override: Option<PathBuf>,
    pub mono_override: Option<PathBuf>,
    pub mono_dll_search_path_override: Option<String>,
    pub mono_debug_enabled: bool,
    pub mono_debug_connect: bool,
    pub mono_debug_suspend: bool,
    pub mono_debug_address: Option<String>,
    pub clr_runtime_coreclr_path: Option<PathBuf>,
    pub clr_corlib_dir: Option<PathBuf>,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            enabled: true,
            ignore_disabled_env: false,
            redirect_output_log: false,
            target_assembly: None,
            boot_config_override: None,
            mono_override: None,
            mono_dll_search_path_override: None,
            mono_debug_enabled: false,
            mono_debug_connect: false,
            mono_debug_suspend: false,
            mono_debug_address: Some("127.0.0.1:10000".to_string()),
            clr_runtime_coreclr_path: None,
            clr_corlib_dir: None,
        }
    }
}

fn parse_text_base(text: Option<impl AsRef<str>>, value: &mut Option<String>) -> bool {
    if let Some(text) = text {
        let text = text.as_ref();
        if !text.is_empty() {
            *value = Some(text.to_string());
            return true;
        }
    }

    false
}

fn parse_path_base(text: Option<impl AsRef<str>>, value: &mut Option<PathBuf>) -> bool {
    if let Some(text) = text {
        let text = text.as_ref();
        if !text.is_empty() {
            *value = Some(path::absolute(Path::new(text)).unwrap());
            return true;
        }
    }

    false
}

fn parse_bool_base(text: Option<impl AsRef<str>>, value: &mut bool) -> bool {
    if let Some(text) = text {
        let text = text.as_ref();
        match text.to_lowercase().as_str() {
            "true" => {
                *value = true;
                return true;
            }
            "false" => {
                *value = false;
                return true;
            }
            _ => {}
        }
    }

    false
}

impl Config {
    pub(crate) fn load() -> Config {
        let mut config = Config::default();

        config.load_from_file();
        config.load_from_environment();
        config.load_from_command_line();

        if env::var_os("MONO_ARGUMENTS").is_some() {
            config.mono_debug_enabled = true;
        }

        config
    }

    fn load_from_file(&mut self) {
        if let Ok(file) = Ini::load_from_file_noescape("doorstop_config.ini") {
            fn parse_text(section: &Properties, key: &str, value: &mut Option<String>) {
                parse_text_base(section.get(key), value);
            }

            fn parse_path(section: &Properties, key: &str, value: &mut Option<PathBuf>) {
                parse_path_base(section.get(key), value);
            }

            fn parse_bool(section: &Properties, key: &str, value: &mut bool) {
                parse_bool_base(section.get(key), value);
            }

            if let Some(section) = file.section(Some("General")) {
                parse_bool(section, "enabled", &mut self.enabled);
                parse_bool(section, "ignore_disable_switch", &mut self.ignore_disabled_env);
                parse_bool(section, "redirect_output_log", &mut self.redirect_output_log);
                parse_path(section, "target_assembly", &mut self.target_assembly);
                parse_path(section, "boot_config_override", &mut self.boot_config_override);
            }

            if let Some(section) = file.section(Some("UnityMono")) {
                parse_path(section, "override", &mut self.mono_override);
                parse_text(section, "dll_search_path_override", &mut self.mono_dll_search_path_override);
                parse_bool(section, "debug_enabled", &mut self.mono_debug_enabled);
                parse_bool(section, "debug_connect", &mut self.mono_debug_connect);
                parse_bool(section, "debug_suspend", &mut self.mono_debug_suspend);
                parse_text(section, "debug_address", &mut self.mono_debug_address);
            }

            if let Some(section) = file.section(Some("Il2Cpp")) {
                parse_path(section, "coreclr_path", &mut self.clr_runtime_coreclr_path);
                parse_path(section, "corlib_dir", &mut self.clr_corlib_dir);
            }
        }
    }

    fn load_from_environment(&mut self) {
        fn parse_text(key: &str, value: &mut Option<String>) {
            parse_text_base(env::var(key).ok(), value);
        }

        fn parse_path(key: &str, value: &mut Option<PathBuf>) {
            parse_path_base(env::var(key).ok(), value);
        }

        fn parse_bool(key: &str, value: &mut bool) {
            parse_bool_base(env::var(key).ok(), value);
        }

        parse_bool("DOORSTOP_ENABLED", &mut self.enabled);
        parse_bool("DOORSTOP_REDIRECT_OUTPUT_LOG", &mut self.redirect_output_log);
        parse_bool("DOORSTOP_IGNORE_DISABLED_ENV", &mut self.ignore_disabled_env);
        parse_bool("DOORSTOP_MONO_DEBUG_ENABLED", &mut self.mono_debug_enabled);
        parse_bool("DOORSTOP_MONO_DEBUG_CONNECT", &mut self.mono_debug_connect);
        parse_bool("DOORSTOP_MONO_DEBUG_SUSPEND", &mut self.mono_debug_suspend);
        parse_text("DOORSTOP_MONO_DEBUG_ADDRESS", &mut self.mono_debug_address);
        parse_path("DOORSTOP_TARGET_ASSEMBLY", &mut self.target_assembly);
        parse_path("DOORSTOP_BOOT_CONFIG_OVERRIDE", &mut self.boot_config_override);
        parse_path("DOORSTOP_MONO_OVERRIDE", &mut self.mono_override);
        parse_text("DOORSTOP_MONO_DLL_SEARCH_PATH_OVERRIDE", &mut self.mono_dll_search_path_override);
        parse_path("DOORSTOP_CLR_RUNTIME_CORECLR_PATH", &mut self.clr_runtime_coreclr_path);
        parse_path("DOORSTOP_CLR_CORLIB_DIR", &mut self.clr_corlib_dir);
    }

    fn load_from_command_line(&mut self) {
        fn parse_text(args: &mut Peekable<Args>, value: &mut Option<String>) {
            if parse_text_base(args.peek(), value) {
                args.next();
            }
        }

        fn parse_path(args: &mut Peekable<Args>, value: &mut Option<PathBuf>) {
            if parse_path_base(args.peek(), value) {
                args.next();
            }
        }

        fn parse_bool(args: &mut Peekable<Args>, value: &mut bool) {
            if parse_bool_base(args.peek(), value) {
                args.next();
            }
        }

        let mut args = env::args().peekable();
        while let Some(name) = args.next() {
            match name.to_lowercase().as_str() {
                "--doorstop-enabled" => parse_bool(&mut args, &mut self.enabled),
                "--doorstop-redirect-output-log" => parse_bool(&mut args, &mut self.redirect_output_log),
                "--doorstop-target-assembly" => parse_path(&mut args, &mut self.target_assembly),
                "--doorstop-boot-config-override" => parse_path(&mut args, &mut self.boot_config_override),
                "--doorstop-mono-override" => parse_path(&mut args, &mut self.mono_override),
                "--doorstop-mono-dll-search-path-override" => parse_text(&mut args, &mut self.mono_dll_search_path_override),
                "--doorstop-mono-debug-enabled" => parse_bool(&mut args, &mut self.mono_debug_enabled),
                "--doorstop-mono-debug-connect" => parse_bool(&mut args, &mut self.mono_debug_connect),
                "--doorstop-mono-debug-suspend" => parse_bool(&mut args, &mut self.mono_debug_suspend),
                "--doorstop-mono-debug-address" => parse_text(&mut args, &mut self.mono_debug_address),
                "--doorstop-clr-corlib-dir" => parse_path(&mut args, &mut self.clr_corlib_dir),
                "--doorstop-clr-runtime-coreclr-path" => parse_path(&mut args, &mut self.clr_runtime_coreclr_path),
                _ => {}
            }
        }
    }
}
