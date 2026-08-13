# RPM spec for ai-assistant (Development_plan Phase 20 "Packaging" -- RPM).
#
# Packages an already-built release binary (Source0 below), the same
# practical approach scripts/package/build-deb.sh takes -- rpmbuild
# doesn't invoke `cargo build` itself. See
# scripts/package/build-rpm.sh for how the sources get assembled and
# docs/ROADMAP.md's Phase 20 notes for why the real binary can't be
# built in this sandbox (Slint/Candle, same as every phase since 2) and
# how this spec was verified anyway (rpmbuild run for real against a
# stand-in binary).

Name:           ai-assistant
Version:        %{_ai_assistant_version}
Release:        1%{?dist}
Summary:        Local Linux AI Assistant

License:        MIT OR Apache-2.0
URL:            https://github.com/lion/ai-assistant
Source0:        ai-assistant
Source1:        ai-assistant.desktop
Source2:        ai-assistant-16.png
Source3:        ai-assistant-32.png
Source4:        ai-assistant-48.png
Source5:        ai-assistant-64.png
Source6:        ai-assistant-128.png
Source7:        ai-assistant-256.png
Source8:        ai-assistant-512.png
Source9:        copyright.txt

BuildArch:      %{_arch}
# Best-effort, same caveat as the .deb Depends list: not confirmed via
# `ldd` against a real binary, since one has never been built here.
Requires:       glibc

%description
A local-first AI assistant for Linux desktops. Chat interface, safe
system control through a permission-gated tool layer, local memory, and
optional voice interaction -- no cloud dependency for the core assistant
(PRD SS1).

Voice features (Phase 12) additionally need espeak-ng and pocketsphinx
installed.

%prep
# Nothing to unpack -- the sources are already-built artifacts, not a
# tarball of source code to configure/compile.

%build
# Nothing to compile here; see the header comment above.

%install
rm -rf %{buildroot}
install -Dm755 %{SOURCE0} %{buildroot}%{_bindir}/ai-assistant
install -Dm644 %{SOURCE1} %{buildroot}%{_datadir}/applications/ai-assistant.desktop
install -Dm644 %{SOURCE2} %{buildroot}%{_datadir}/icons/hicolor/16x16/apps/ai-assistant.png
install -Dm644 %{SOURCE3} %{buildroot}%{_datadir}/icons/hicolor/32x32/apps/ai-assistant.png
install -Dm644 %{SOURCE4} %{buildroot}%{_datadir}/icons/hicolor/48x48/apps/ai-assistant.png
install -Dm644 %{SOURCE5} %{buildroot}%{_datadir}/icons/hicolor/64x64/apps/ai-assistant.png
install -Dm644 %{SOURCE6} %{buildroot}%{_datadir}/icons/hicolor/128x128/apps/ai-assistant.png
install -Dm644 %{SOURCE7} %{buildroot}%{_datadir}/icons/hicolor/256x256/apps/ai-assistant.png
install -Dm644 %{SOURCE8} %{buildroot}%{_datadir}/icons/hicolor/512x512/apps/ai-assistant.png
install -Dm644 %{SOURCE9} %{buildroot}%{_datadir}/doc/ai-assistant/copyright.txt

%files
%{_bindir}/ai-assistant
%{_datadir}/applications/ai-assistant.desktop
%{_datadir}/icons/hicolor/16x16/apps/ai-assistant.png
%{_datadir}/icons/hicolor/32x32/apps/ai-assistant.png
%{_datadir}/icons/hicolor/48x48/apps/ai-assistant.png
%{_datadir}/icons/hicolor/64x64/apps/ai-assistant.png
%{_datadir}/icons/hicolor/128x128/apps/ai-assistant.png
%{_datadir}/icons/hicolor/256x256/apps/ai-assistant.png
%{_datadir}/icons/hicolor/512x512/apps/ai-assistant.png
%doc %{_datadir}/doc/ai-assistant/copyright.txt

%post
if command -v update-desktop-database >/dev/null 2>&1; then
    update-desktop-database -q %{_datadir}/applications || :
fi
if command -v gtk-update-icon-cache >/dev/null 2>&1; then
    gtk-update-icon-cache -q -t -f %{_datadir}/icons/hicolor || :
fi

%postun
if command -v update-desktop-database >/dev/null 2>&1; then
    update-desktop-database -q %{_datadir}/applications || :
fi
if command -v gtk-update-icon-cache >/dev/null 2>&1; then
    gtk-update-icon-cache -q -t -f %{_datadir}/icons/hicolor || :
fi

%changelog
* Wed Aug 12 2026 Lion <noreply@example.invalid> - 0.1.0-1
- Initial packaging (Development_plan Phase 20).
