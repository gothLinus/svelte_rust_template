## Dateien: wie Name, Typ und Größe eines Uploads aussehen müssen und was einen Upload stoppt.
## Jeder hat seinen eigenen stabilen `code` (im Kommentar), auf den Clients reagieren können.

# too_long. { $max } ist eine Anzahl von Zeichen.
file-name-too-long = darf höchstens { $max } Zeichen lang sein
# invalid_characters: Steuerzeichen, Schrägstriche oder ein Name, der nur aus Punkten besteht.
file-name-invalid = darf keine Schrägstriche oder Steuerzeichen enthalten
# invalid_content_type: der Content-Type des Uploads ist kein Medientyp wie image/png.
file-content-type-invalid = ist kein gültiger Medientyp, etwa image/png
# too_large. { $max } ist eine Anzahl von Megabyte (1 MB = 1.048.576 Byte).
file-too-large = darf höchstens { $max } MB groß sein
# file_quota_exceeded: der Upload würde nicht in den Platz passen, der dem Konto bleibt.
conflict-file-quota-exceeded = es ist nicht mehr genug Platz für diese Datei; löschen Sie Dateien, um Platz zu schaffen
