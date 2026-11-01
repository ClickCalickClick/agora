# Agora: Meson drives Cargo and installs data files.
#
# crates/agora-app/build.rs compiles the Blueprint files, the GResource bundle
# and a development copy of the GSettings schema, so Meson does not compile
# resources itself; it passes the build configuration to Cargo through
# environment variables (AGORA_APP_ID, AGORA_PROFILE, AGORA_LOCALEDIR,
# AGORA_PKGDATADIR), installs the `agora` binary and installs data files.
project(
  'agora',
  version: '0.1.0',
  license: 'GPL-3.0-or-later',
  meson_version: '>= 1.4.0',
)

i18n = import('i18n')
gnome = import('gnome')

base_id = 'io.github.ClickCalickClick.Agora'
gettext_package = meson.project_name()

# Target runtime (SPEC §3.1). WebKitGTK 6.0 is required from slice 2.
dependency('glib-2.0', version: '>= 2.84')
dependency('gio-2.0', version: '>= 2.84')
dependency('gtk4', version: '>= 4.24')
dependency('libadwaita-1', version: '>= 1.10')

cargo = find_program('cargo', required: true)
find_program('blueprint-compiler', required: true)
find_program('glib-compile-schemas', required: true)
desktop_file_validate = find_program('desktop-file-validate', required: false)
appstreamcli = find_program('appstreamcli', required: false)
python = import('python').find_installation('python3')

if get_option('profile') == 'development'
  profile = 'Devel'
  application_id = '@0@.@1@'.format(base_id, profile)
else
  profile = ''
  application_id = base_id
endif

prefix = get_option('prefix')
bindir = prefix / get_option('bindir')
localedir = prefix / get_option('localedir')
datadir = prefix / get_option('datadir')
pkgdatadir = datadir / meson.project_name()

subdir('data')
subdir('po')

# Cargo build of the `agora` binary (crates/agora-app). There is no src/
# directory; the Cargo workspace at the repository root is built as a whole.
cargo_target_dir = meson.project_build_root() / 'cargo-target'
cargo_manifest = meson.project_source_root() / 'Cargo.toml'
cargo_env = {
  'CARGO_TARGET_DIR': cargo_target_dir,
  'AGORA_APP_ID': application_id,
  'AGORA_PROFILE': get_option('profile'),
  'AGORA_LOCALEDIR': localedir,
  'AGORA_PKGDATADIR': pkgdatadir,
}

custom_target(
  'cargo-build',
  output: meson.project_name(),
  command: [
    files('build-aux/cargo-build.sh'),
    cargo,
    cargo_manifest,
    cargo_target_dir,
    get_option('profile'),
    '@OUTPUT@',
  ],
  env: cargo_env,
  console: true,
  build_by_default: true,
  build_always_stale: true,
  install: true,
  install_dir: bindir,
)

# Tests. Data validators are in the `data` suite (see data/meson.build).
test(
  'cargo-test',
  cargo,
  args: ['test', '--workspace', '--manifest-path', cargo_manifest, '--no-fail-fast'],
  env: cargo_env,
  timeout: 1800,
  is_parallel: false,
  suite: 'rust',
)

test(
  'potfiles',
  python,
  args: [files('build-aux/gen-potfiles.py'), '--check'],
  suite: 'data',
)

gnome.post_install(
  gtk_update_icon_cache: true,
  glib_compile_schemas: true,
  update_desktop_database: true,
)

summary(
  {
    'Profile': get_option('profile'),
    'Application ID': application_id,
    'Prefix': prefix,
  },
  section: 'Agora',
)
