from pathlib import Path
from xml.sax.saxutils import escape
from zipfile import ZIP_DEFLATED, ZipFile


SHEETS = {
    "Machines": [
        ["machine_id", "machine_name", "initial_state_id", "step_order", "description"],
        ["AC", "空調制御", "MANUAL", "1", "空調モード制御"],
        ["LINK", "連携制御", "OFF", "2", "空調とヒーターの連携"],
        ["HEATER", "ヒーター制御", "OFF", "3", "ヒーター状態"],
    ],
    "States": [
        ["machine_id", "state_id", "state_name"],
        ["AC", "MANUAL", "マニュアル"],
        ["AC", "AUTO", "オート"],
        ["LINK", "OFF", "停止"],
        ["LINK", "ON", "稼働"],
        ["HEATER", "OFF", "停止"],
        ["HEATER", "ON", "稼働"],
    ],
    "Inputs": [
        ["input_id", "input_name", "input_type", "allowed_values", "default_value", "unit"],
        ["AUTO_SW", "AUTOスイッチ", "BOOL", "OFF|ON", "OFF", ""],
        ["LINK_ENABLE", "連携有効", "BOOL", "OFF|ON", "ON", ""],
        ["SAFETY_LOCK", "安全ロック", "BOOL", "OFF|ON", "OFF", ""],
    ],
    "Outputs": [
        ["output_id", "owner_machine_id", "output_name", "output_type", "allowed_values", "default_previous_value"],
        ["HEATER_CMD", "HEATER", "ヒーター指令", "BOOL", "OFF|ON", "OFF"],
    ],
    "Events": [
        ["event_id", "event_name", "input_id", "from_value", "to_value", "enabled"],
        ["AUTO_SW_UP", "AUTOスイッチ ON", "AUTO_SW", "OFF", "ON", "TRUE"],
        ["AUTO_SW_DOWN", "AUTOスイッチ OFF", "AUTO_SW", "ON", "OFF", "TRUE"],
        ["LINK_ENABLE_UP", "連携スイッチ ON", "LINK_ENABLE", "OFF", "ON", "TRUE"],
        ["LINK_ENABLE_DOWN", "連携スイッチ OFF", "LINK_ENABLE", "ON", "OFF", "TRUE"],
    ],
    "Transitions": [
        ["rule_id", "machine_id", "from_state_id", "guard", "to_state_id", "priority", "source_ref", "description"],
        ["AC_001", "AC", "MANUAL", "RISE(AUTO_SW)", "AUTO", "10", "sheet1!A1", "AUTOに切り替える"],
        ["AC_002", "AC", "AUTO", "FALL(AUTO_SW)", "MANUAL", "10", "sheet1!A2", "MANUALに切り替える"],
        ["LINK_001", "LINK", "OFF", "STATE.AC == AUTO AND INPUT.LINK_ENABLE == ON", "ON", "10", "sheet2!B1", "条件成立で連携"],
        ["LINK_002", "LINK", "ON", "STATE.AC == MANUAL OR INPUT.LINK_ENABLE == OFF", "OFF", "10", "sheet2!B2", "条件成立で解除"],
        ["HEATER_001", "HEATER", "OFF", "STATE.LINK == ON AND PREV.OUTPUT.HEATER_CMD == OFF AND INPUT.SAFETY_LOCK == OFF", "ON", "20", "sheet3!C1", "前回出力がOFFなら起動"],
        ["HEATER_002", "HEATER", "ON", "STATE.LINK == OFF OR INPUT.SAFETY_LOCK == ON", "OFF", "10", "sheet3!C2", "連携解除で停止"],
    ],
}


def column_name(index: int) -> str:
    label = ""
    while index:
        index, remainder = divmod(index - 1, 26)
        label = chr(65 + remainder) + label
    return label


def worksheet_xml(rows: list[list[str]]) -> str:
    xml_rows = []
    for row_index, row in enumerate(rows, start=1):
        cells = []
        for column_index, value in enumerate(row, start=1):
            if value == "":
                continue
            reference = f"{column_name(column_index)}{row_index}"
            cells.append(
                f'<c r="{reference}" t="inlineStr"><is><t>{escape(value)}</t></is></c>'
            )
        xml_rows.append(f'<row r="{row_index}">{"".join(cells)}</row>')
    return (
        '<?xml version="1.0" encoding="UTF-8" standalone="yes"?>'
        '<worksheet xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main">'
        f'<sheetData>{"".join(xml_rows)}</sheetData></worksheet>'
    )


def write_workbook(path: Path) -> None:
    sheet_names = list(SHEETS)
    content_overrides = [
        '<Override PartName="/xl/workbook.xml" ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.sheet.main+xml"/>'
    ]
    relationships = []
    sheet_entries = []
    for index, name in enumerate(sheet_names, start=1):
        content_overrides.append(
            f'<Override PartName="/xl/worksheets/sheet{index}.xml" ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.worksheet+xml"/>'
        )
        relationships.append(
            f'<Relationship Id="rId{index}" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/worksheet" Target="worksheets/sheet{index}.xml"/>'
        )
        sheet_entries.append(
            f'<sheet name="{name}" sheetId="{index}" r:id="rId{index}"/>'
        )

    with ZipFile(path, "w", ZIP_DEFLATED) as archive:
        archive.writestr(
            "[Content_Types].xml",
            '<?xml version="1.0" encoding="UTF-8" standalone="yes"?>'
            '<Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types">'
            '<Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/>'
            '<Default Extension="xml" ContentType="application/xml"/>'
            f'{"".join(content_overrides)}</Types>',
        )
        archive.writestr(
            "_rels/.rels",
            '<?xml version="1.0" encoding="UTF-8" standalone="yes"?>'
            '<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">'
            '<Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="xl/workbook.xml"/>'
            '</Relationships>',
        )
        archive.writestr(
            "xl/workbook.xml",
            '<?xml version="1.0" encoding="UTF-8" standalone="yes"?>'
            '<workbook xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main" '
            'xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships">'
            f'<sheets>{"".join(sheet_entries)}</sheets></workbook>',
        )
        archive.writestr(
            "xl/_rels/workbook.xml.rels",
            '<?xml version="1.0" encoding="UTF-8" standalone="yes"?>'
            '<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">'
            f'{"".join(relationships)}</Relationships>',
        )
        for index, name in enumerate(sheet_names, start=1):
            archive.writestr(f"xl/worksheets/sheet{index}.xml", worksheet_xml(SHEETS[name]))


if __name__ == "__main__":
    fixture_path = Path(__file__).with_name("golden-workbook.xlsx")
    write_workbook(fixture_path)
    print(f"Wrote {fixture_path}")
