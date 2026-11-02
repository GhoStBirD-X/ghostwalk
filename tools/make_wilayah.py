"""Builds assets/wilayah.tsv.gz, the offline list of kabupaten/kota and kecamatan used to pick map areas.

Source: GeoNames (CC BY 4.0), https://download.geonames.org/export/dump/ID.zip (unzipped ID.txt).
Usage: python3 tools/make_wilayah.py path/to/ID.txt assets/wilayah.tsv.gz

Each output line: kind (K = kabupaten/kota, C = kecamatan), name, context, latitude, longitude.
Many kecamatan in GeoNames do not say which kabupaten/kota they belong to; those get the nearest one
in the same province, written as "dekat ..." so the label never claims a boundary it does not know.
"""
import gzip, math, sys

PROVINCES = {
    "01": "Aceh", "02": "Bali", "03": "Bengkulu", "04": "DKI Jakarta", "05": "Jambi", "07": "Jawa Tengah",
    "08": "Jawa Timur", "10": "DI Yogyakarta", "11": "Kalimantan Barat", "12": "Kalimantan Selatan",
    "13": "Kalimantan Tengah", "14": "Kalimantan Timur", "15": "Lampung", "17": "Nusa Tenggara Barat",
    "18": "Nusa Tenggara Timur", "21": "Sulawesi Tengah", "22": "Sulawesi Tenggara", "24": "Sumatera Barat",
    "26": "Sumatera Utara", "28": "Maluku", "29": "Maluku Utara", "30": "Jawa Barat", "31": "Sulawesi Utara",
    "32": "Sumatera Selatan", "33": "Banten", "34": "Gorontalo", "35": "Kepulauan Bangka Belitung",
    "36": "Papua", "37": "Riau", "38": "Sulawesi Selatan", "39": "Papua Barat", "40": "Kepulauan Riau",
    "41": "Sulawesi Barat", "42": "Kalimantan Utara", "PD": "Papua Barat Daya", "PT": "Papua Tengah",
    "PE": "Papua Pegunungan", "PS": "Papua Selatan",
}


def short(name):
    for long, s in (("Kota Administrasi ", "Kota "), ("Kabupaten Administrasi ", "Kab. "),
                    ("Kabupaten ", "Kab. "), ("Kecamatan ", "Kec. ")):
        if name.startswith(long):
            return s + name[len(long):]
    return name


def main(src, out):
    regencies, districts = [], []
    for line in open(src, encoding="utf-8"):
        f = line.rstrip("\n").split("\t")
        if len(f) < 13 or f[6] != "A" or f[10] not in PROVINCES:
            continue
        row = (short(f[1]), float(f[4]), float(f[5]), f[10], f[11])
        if f[7] == "ADM2":
            regencies.append(row)
        elif f[7] == "ADM3":
            districts.append(row)
    by_code = {(r[3], r[4]): r[0] for r in regencies if r[4]}

    def nearest(lat, lon, a1):
        best, dist = None, 1e9
        k = math.cos(math.radians(lat))
        for name, la, lo, ra1, _ in regencies:
            if ra1 == a1:
                d = (la - lat) ** 2 + ((lo - lon) * k) ** 2
                if d < dist:
                    best, dist = name, d
        return best

    lines, seen = [], set()
    for name, lat, lon, a1, _ in regencies:
        lines.append(("K", name, PROVINCES[a1], lat, lon))
    for name, lat, lon, a1, a2 in districts:
        reg = by_code.get((a1, a2))
        if reg:
            ctx = f"{reg}, {PROVINCES[a1]}"
        else:
            near = nearest(lat, lon, a1)
            ctx = f"dekat {near}, {PROVINCES[a1]}" if near else PROVINCES[a1]
        lines.append(("C", name, ctx, lat, lon))
    with gzip.open(out, "wt", encoding="utf-8") as o:
        for kind, name, ctx, lat, lon in lines:
            if (name, ctx) in seen:
                continue
            seen.add((name, ctx))
            o.write(f"{kind}\t{name}\t{ctx}\t{lat:.5f}\t{lon:.5f}\n")
    print(f"{len(seen)} wilayah -> {out}")


if __name__ == "__main__":
    main(sys.argv[1], sys.argv[2])
