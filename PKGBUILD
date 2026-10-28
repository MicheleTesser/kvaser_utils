pkgbase=kvaser-utils
pkgname=('kvaser-utils' 'kvaser-linuxcan-dkms')
pkgver=0.1.0
pkgrel=2
pkgdesc="Kvaser Memorator Pro 2 utilities and TUI"
arch=('x86_64')
url="https://github.com/MicheleTesser/kvaser_utils"
license=('Apache-2.0' 'BSD-3-Clause' 'GPL-2.0-or-later')
makedepends=('cargo' 'clang' 'make' 'perl' 'pkgconf' 'libxml2')
options=('!lto' '!debug')
source=("$pkgbase-$pkgver.tar.gz")
# Local release archive generated from the working tree by build-packages.sh.
sha256sums=('SKIP')

prepare() {
  cd "$srcdir/$pkgbase-$pkgver"
  tar -xf third_party/kvaser/linuxcan_5_51_461.tar.gz -C third_party/kvaser
  tar -xf third_party/kvaser/kvlibsdk_5_51_461.tar.gz -C third_party/kvaser
  patch -p1 -d third_party/kvaser/linuxcan < scripts/patches/linuxcan-5.51-modern-kernel.patch
  # Private libraries must resolve their siblings without searching the cwd.
  find third_party/kvaser/linuxcan third_party/kvaser/kvlibsdk -name Makefile \
    -exec sed -i 's@ -Wl,-rpath=\. @ @g; s@ -shared @ -shared -Wl,-z,relro,-z,now -Wl,-rpath,/usr/lib/kvaser_utils @g' {} +
}

build() {
  cd "$srcdir/$pkgbase-$pkgver"
  # Portable x86-64 binaries, including on hosts configured with -march=native.
  export CFLAGS='-O2 -pipe -march=x86-64 -mtune=generic'
  export CXXFLAGS="$CFLAGS"
  export RUSTFLAGS='-C target-cpu=x86-64'
  export CARGO_TARGET_DIR="$srcdir/target"
  export PATH="/usr/bin:$PATH"
  export CC=/usr/bin/gcc CXX=/usr/bin/g++
  export LIBCLANG_PATH=/usr/lib
  bash scripts/build-kvaser-local.sh
  cargo build --release --locked -p kvaser-tui
}

package_kvaser-utils() {
  depends=('gcc-libs' 'glibc' 'libxml2' 'libxml2.so' 'zlib' 'iproute2' 'sudo' 'kmod' 'bash' "kvaser-linuxcan-dkms=$pkgver-$pkgrel")
  install=packaging/kvaser-utils.install
  cd "$srcdir/$pkgbase-$pkgver"
  install -Dm755 "$srcdir/target/release/kvaser-tui" "$pkgdir/usr/bin/kvaser-tui"
  install -Dm755 scripts/kvaser_mode "$pkgdir/usr/bin/kvaser_mode"
  install -Dm755 scripts/kvaser-driver-mode.sh "$pkgdir/usr/lib/kvaser_utils/kvaser-driver-mode.sh"
  install -Dm755 scripts/setup-socketcan-tunnel-ifaces.sh "$pkgdir/usr/bin/kvaser_socketcan_ifaces"
  install -d "$pkgdir/usr/lib/kvaser_utils"
  local library
  for library in \
    linuxcan/canlib/libcanlib.so \
    kvlibsdk/kvadblib/so.ndb/libkvadblib.so \
    kvlibsdk/kvamemolibxml/so.ndb/libkvamemolibxml.so \
    kvlibsdk/kvmlib/kvmlib/so.ndb/libkvmlib.so \
    kvlibsdk/kvmlib/memolib_wrap/so.ndb/libkvamemolib.so \
    kvlibsdk/kvmlib/memolib_fd/so.ndb/libkvamemolib0700.so \
    kvlibsdk/kvlclib/so.ndb/libkvlclib.so; do
    cp -d third_party/kvaser/"$library"* "$pkgdir/usr/lib/kvaser_utils/"
  done
  install -Dm644 README.md "$pkgdir/usr/share/doc/$pkgname/README.md"
  install -Dm644 third_party/kvaser/kvlibsdk/COPYING.BSD "$pkgdir/usr/share/licenses/$pkgname/Kvaser-BSD"
}

package_kvaser-linuxcan-dkms() {
  pkgdesc="Kvaser LinuxCAN kvcommon/mhydra drivers rebuilt automatically with DKMS"
  license=('BSD-3-Clause' 'GPL-2.0-or-later')
  depends=('dkms' 'clang' 'llvm' 'lld')
  local dkms_version="$pkgver-$pkgrel"
  local destination="$pkgdir/usr/src/kvaser-linuxcan-$dkms_version"
  cd "$srcdir/$pkgbase-$pkgver"
  install -d "$destination"
  cp -r third_party/kvaser/linuxcan/{common,mhydra,include,dkms} "$destination/"
  cp third_party/kvaser/linuxcan/{Makefile,config.mak,moduleinfo.txt,COPYING,COPYING.BSD,COPYING.GPL} "$destination/"
  sed "s/@VERSION@/$dkms_version/g" packaging/dkms.conf > "$destination/dkms.conf"
  install -m755 packaging/dkms-build.sh "$destination/dkms-build.sh"
  install -Dm644 packaging/kvaser-linuxcan.conf "$pkgdir/usr/lib/modprobe.d/kvaser-linuxcan-mode.conf"
  install -Dm644 third_party/kvaser/linuxcan/COPYING.BSD "$pkgdir/usr/share/licenses/$pkgname/Kvaser-BSD"
}
