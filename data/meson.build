# Data files: desktop entry, AppStream metainfo, GSettings schema and icons.
# UI files, stylesheets and in-app icons under data/resources/ are compiled
# into the binary by crates/agora-app/build.rs, not here.

# Desktop entry
desktop_conf = configuration_data()
desktop_conf.set('app_id', application_id)
desktop_file = i18n.merge_file(
  type: 'desktop',
  input: configure_file(
    input: '@0@.desktop.in.in'.format(base_id),
    output: '@0@.desktop.in'.format(application_id),
    configuration: desktop_conf,
  ),
  output: '@0@.desktop'.format(application_id),
  po_dir: meson.project_source_root() / 'po',
  install: true,
  install_dir: get_option('datadir') / 'applications',
)

if desktop_file_validate.found()
  test(
    'validate-desktop',
    desktop_file_validate,
    args: [desktop_file.full_path()],
    depends: desktop_file,
    suite: 'data',
  )
endif

# AppStream metainfo
metainfo_conf = configuration_data()
metainfo_conf.set('app_id', application_id)
metainfo_conf.set('gettext_package', gettext_package)
metainfo_file = i18n.merge_file(
  input: configure_file(
    input: '@0@.metainfo.xml.in.in'.format(base_id),
    output: '@0@.metainfo.xml.in'.format(application_id),
    configuration: metainfo_conf,
  ),
  output: '@0@.metainfo.xml'.format(application_id),
  po_dir: meson.project_source_root() / 'po',
  install: true,
  install_dir: get_option('datadir') / 'metainfo',
)

if appstreamcli.found()
  test(
    'validate-metainfo',
    appstreamcli,
    args: ['validate', '--no-net', '--explain', metainfo_file.full_path()],
    depends: metainfo_file,
    suite: 'data',
  )
endif

# GSettings schema. The .in file has no placeholders (build.rs copies it
# verbatim), and the Devel build installs the same schema ID (SPEC §13).
schema_file = configure_file(
  input: '@0@.gschema.xml.in'.format(base_id),
  output: '@0@.gschema.xml'.format(base_id),
  copy: true,
  install: true,
  install_dir: get_option('datadir') / 'glib-2.0' / 'schemas',
)

test(
  'validate-schema',
  find_program('glib-compile-schemas'),
  args: ['--strict', '--dry-run', meson.current_build_dir()],
  suite: 'data',
)

# App icons. Temporary placeholder art; slice 6 redesigns the icon (SPEC §2.1).
install_data(
  'icons/hicolor/scalable/apps/@0@.svg'.format(application_id),
  install_dir: get_option('datadir') / 'icons' / 'hicolor' / 'scalable' / 'apps',
)
install_data(
  'icons/hicolor/symbolic/apps/@0@-symbolic.svg'.format(base_id),
  install_dir: get_option('datadir') / 'icons' / 'hicolor' / 'symbolic' / 'apps',
  rename: '@0@-symbolic.svg'.format(application_id),
)
