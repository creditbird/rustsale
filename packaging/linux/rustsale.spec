# RPM spec for the RustSale Linux (GTK 4) release. Repackages the output of `nsh package --target linux`.
# Build: packaging/linux/build-rpm.sh <nsh-output-dir> <out-dir>
Name:           rustsale
Version:        %{?pkg_version}%{!?pkg_version:0.1.0}
Release:        1%{?dist}
Summary:        Sales CRM for customer conversations and outreach
License:        Proprietary
BuildArch:      x86_64
AutoReqProv:    no
# A prebuilt runner: nothing to strip or split into debuginfo.
%global debug_package %{nil}
%global __strip /bin/true
# The runner links WebKitGTK 6.0 and GTK 4 at load time (DT_NEEDED), even though the app opens no web view.
Requires:       webkitgtk6.0
Requires:       gtk4 >= 4.14
Source0:        rustsale-root.tar.gz

%description
RustSale is a native (GTK 4) desktop app for customer conversations, outreach and lead management.

%prep
%setup -q -c -T
tar -xzf %{SOURCE0}

%build

%install
cp -a . %{buildroot}/

%files
/opt/rustsale
/usr/bin/rustsale
/usr/share/applications/com.rustsale.crm.v3.desktop
/usr/share/icons/hicolor/512x512/apps/rustsale.png

%changelog
* Mon Nov 23 2026 RustSale Packagers <packaging@rustsale.invalid> - 0.1.0-1
- First Linux package: GTK 4 runner plus its declared dependencies
