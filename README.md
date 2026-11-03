# GhostWalk

Pencatat olahraga yang **pribadi dan offline** untuk Android dan komputer (Linux). Tanpa akun, tanpa iklan, tanpa pelacak. Semua aktivitas hanya tersimpan di perangkat Anda.

## Fitur

- **Rekam aktivitas**: lari, sepeda, jalan, hiking, renang, latihan. Di HP memakai GPS dan tetap jalan saat layar mati, di komputer berupa stopwatch.
- **Saat merekam**: suara tiap kilometer (Bahasa Indonesia), jeda otomatis saat berhenti, hitung langkah, latihan interval dengan getar dan suara, layar fokus dengan angka besar dan "geser untuk selesai".
- **Peta offline**: unduh peta per kecamatan atau kabupaten/kota (bisa beberapa wilayah sekaligus), lalu pakai tanpa internet. Bisa juga impor file `.pmtiles` sendiri.
- **Statistik**: ringkasan mingguan, ringkasan bulanan/tahunan, rekor pribadi, kalender latihan, split per km, grafik tempo, zona detak jantung.
- **Rekor dan rute**: perayaan saat memecahkan rekor atau mencapai target, dan perbandingan waktu di rute yang sama.
- **Bagikan**: kartu gambar berisi rute dan angka, untuk dikirim lewat app apa pun.
- **Target dan perlengkapan**: target mingguan/bulanan, jarak tempuh sepatu dan sepeda.
- **Rute**: buat rute sendiri, lalu lari mengikutinya: rute tampil di peta saat merekam, dengan sisa jarak dan peringatan (suara dan getar) saat keluar rute. Peta jejak semua aktivitas.
- **Lawan hantu**: berlomba dengan lari Anda sebelumnya. Titik hantu bergerak di peta, selisih detik terlihat setiap saat, dan di setiap km terdengar apakah Anda di depan atau di belakang hantu, juga saat layar mati.
- **Data milik Anda**: ekspor GPX dan CSV, cadangkan dan pulihkan dalam JSON.
- Tema gelap dan terang, bahasa Indonesia.

## Privasi

- Tidak perlu akun, email, nomor HP, atau tanggal lahir.
- Internet **hanya** dipakai saat Anda menekan tombol unduh peta. Yang diminta hanyalah potongan peta untuk wilayah yang Anda pilih sendiri dari daftar; lokasi GPS, rute, dan aktivitas tidak pernah dikirim ke mana pun.
- Izin Android yang dipakai: lokasi (merekam rute), notifikasi dan layanan latar depan (rekam saat layar mati), aktivitas fisik (hitung langkah), getar (penanda interval), dan internet (unduh peta).

## Install di Android

1. Unduh `GhostWalk.apk` dari halaman [Releases](../../releases).
2. Buka file tersebut di HP dan izinkan "Instal dari sumber tidak dikenal" bila diminta.
3. Butuh Android 8 ke atas, HP 64-bit (arm64).

## Build dari kode

Butuh [Rust](https://rustup.rs) (stable).

**Komputer (Linux):**

```sh
cargo run --release
```

**Android:** butuh Android NDK, JDK 17+, Android build-tools dan `android.jar` (platform 35). Atur `TOOLS` ke folder yang berisi `ndk/`, `jdk/`, `build-tools/` dan `platform/`, lalu:

```sh
rustup target add aarch64-linux-android
TOOLS=/path/ke/android-tools sh android/build-apk.sh
```

APK tersimpan di `dist/GhostWalk.apk`. Skrip membuat kunci tanda tangan sendiri (`ghostwalk-release.keystore` di `TOOLS`) bila belum ada; simpan kunci itu baik-baik, karena update APK harus ditandatangani dengan kunci yang sama.

**Daftar wilayah** (`assets/wilayah.tsv.gz`) dibuat dari data GeoNames:

```sh
python3 tools/make_wilayah.py ID.txt assets/wilayah.tsv.gz
```

## Kredit dan lisensi data

- Peta: © [OpenStreetMap](https://www.openstreetmap.org/copyright) contributors (ODbL), melalui build [Protomaps](https://protomaps.com).
- Daftar wilayah: [GeoNames](https://www.geonames.org) (CC BY 4.0).
- Font: [Barlow](https://github.com/jpt/barlow) (SIL Open Font License 1.1, lihat `assets/fonts/OFL.txt`).
