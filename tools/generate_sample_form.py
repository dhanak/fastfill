"""Generate the printable FastFill sample form without external packages."""

from __future__ import annotations

import sys
from pathlib import Path

PAGE_WIDTH = 595.28
PAGE_HEIGHT = 841.89
LEFT = 40.0
RIGHT = 555.0


def number(value: float) -> str:
    return f"{value:.2f}".rstrip("0").rstrip(".")


def escape_text(value: str) -> str:
    return value.replace("\\", "\\\\").replace("(", "\\(").replace(")", "\\)")


class PdfCanvas:
    def __init__(self) -> None:
        self.commands: list[str] = []

    @staticmethod
    def pdf_y(y: float) -> float:
        return PAGE_HEIGHT - y

    def text(
        self,
        value: str,
        x: float,
        y: float,
        size: float = 8.6,
        *,
        bold: bool = False,
        gray: float = 0.0,
    ) -> None:
        value.encode("latin-1")
        font = "F2" if bold else "F1"
        self.commands.append(
            f"{number(gray)} g BT /{font} {number(size)} Tf "
            f"1 0 0 1 {number(x)} {number(self.pdf_y(y))} Tm "
            f"({escape_text(value)}) Tj ET"
        )

    def line(
        self,
        x1: float,
        y1: float,
        x2: float,
        y2: float,
        *,
        width: float = 0.8,
        gray: float = 0.0,
        dash: str = "[] 0",
    ) -> None:
        self.commands.append(
            f"{number(gray)} G {number(width)} w {dash} d "
            f"{number(x1)} {number(self.pdf_y(y1))} m "
            f"{number(x2)} {number(self.pdf_y(y2))} l S"
        )

    def rect(
        self,
        x: float,
        y: float,
        width: float,
        height: float,
        *,
        line_width: float = 0.8,
        gray: float = 0.0,
        fill_gray: float | None = None,
    ) -> None:
        bottom = self.pdf_y(y + height)
        geometry = f"{number(x)} {number(bottom)} {number(width)} {number(height)} re"
        if fill_gray is not None:
            self.commands.append(f"q {number(fill_gray)} g {geometry} f Q")
        self.commands.append(
            f"{number(gray)} G {number(line_width)} w [] 0 d {geometry} S"
        )

    def section(self, y: float, title: str) -> None:
        self.rect(
            LEFT,
            y,
            RIGHT - LEFT,
            20,
            fill_gray=0.91,
        )
        self.text(title, LEFT + 8, y + 14, 9.5, bold=True)

    def field(
        self,
        label: str,
        label_x: float,
        y: float,
        line_start: float,
        line_end: float,
        *,
        size: float = 8.6,
        dotted: bool = False,
    ) -> None:
        self.text(label, label_x, y, size)
        dash = "[1.2 2.4] 0" if dotted else "[] 0"
        self.line(line_start, y + 2, line_end, y + 2, dash=dash)

    def checkbox(
        self,
        x: float,
        y: float,
        label: str,
        *,
        small: bool = False,
    ) -> None:
        side = 10 if small else 13
        size = 7.2 if small else 8.6
        gap = 14 if small else 19
        baseline = y + 9 if small else y + 11
        self.rect(x, y, side, side)
        self.text(label, x + gap, baseline, size)

    def content(self) -> bytes:
        return ("\n".join(self.commands) + "\n").encode("latin-1")


def draw_form(canvas: PdfCanvas) -> None:
    canvas.rect(28, 28, 539, 786, line_width=1.15)

    canvas.text(
        "GENERAL SERVICE REQUEST FORM",
        LEFT,
        57,
        16,
        bold=True,
    )
    canvas.text(
        "Complete in block letters. Tick boxes where applicable.",
        LEFT,
        75,
        gray=0.28,
    )
    canvas.text(
        "To correct an entry, strike it through once and write the "
        "correction beside it.",
        LEFT,
        88,
        7.2,
        gray=0.28,
    )

    canvas.rect(411, 39, 144, 53)
    canvas.text("OFFICE USE", 419, 53, bold=True)
    canvas.field("Case no.", 419, 70, 463, 546, size=7.2)
    canvas.field("Received", 419, 85, 463, 546, size=7.2)

    canvas.section(103, "A. APPLICANT DETAILS")
    canvas.field("Full name", 48, 143, 96, 350)
    canvas.field("Date of birth", 368, 143, 433, 547)
    canvas.field("Address", 48, 169, 89, 547)
    canvas.field("City", 48, 195, 72, 298)
    canvas.field("Postal code", 320, 195, 378, 547)
    canvas.field("Phone", 48, 221, 82, 281)
    canvas.field("Email", 301, 221, 335, 547)
    canvas.field("Reference no.", 48, 247, 116, 274)
    canvas.text(
        "Preferred contact (circle one):  EMAIL   PHONE   POST",
        296,
        247,
    )

    canvas.section(262, "B. REQUEST DETAILS")
    canvas.text("Tick all services that apply:", 48, 297)
    canvas.checkbox(48, 307, "New request")
    canvas.checkbox(177, 307, "Update details")
    canvas.checkbox(318, 307, "Appointment")
    canvas.checkbox(445, 307, "Document copy")
    canvas.checkbox(48, 332, "Information only")
    canvas.checkbox(177, 332, "Accessibility support")
    canvas.checkbox(354, 332, "Other")
    canvas.line(398, 345, 547, 345, dash="[1.2 2.4] 0")

    canvas.field(
        "Requested service or subject",
        48,
        373,
        184,
        547,
    )
    canvas.field("Preferred date", 48, 400, 121, 251)
    canvas.field("Time", 276, 400, 305, 382)
    canvas.text(
        "Priority - underline one:  STANDARD   URGENT",
        404,
        400,
        7.2,
    )
    canvas.text(
        "Meeting format - circle one:   IN PERSON   TELEPHONE   VIDEO",
        48,
        427,
    )
    canvas.text(
        "Brief description (highlight any deadline or key requirement):",
        48,
        454,
    )
    canvas.rect(48, 464, 499, 71, line_width=0.55, gray=0.5)
    canvas.line(57, 483, 538, 483, width=0.55, gray=0.5)
    canvas.line(57, 502, 538, 502, width=0.55, gray=0.5)
    canvas.line(57, 521, 538, 521, width=0.55, gray=0.5)

    canvas.section(548, "C. SUPPORTING INFORMATION")
    canvas.text(
        "Mark each statement with a check if correct or an X if it does not apply.",
        48,
        581,
    )
    canvas.checkbox(
        48,
        591,
        "The contact details in section A are current.",
    )
    canvas.checkbox(
        48,
        614,
        "Required supporting documents are attached.",
    )
    canvas.checkbox(
        48,
        637,
        "I require correspondence in an accessible format.",
    )
    canvas.text("Documents attached (tick all):", 335, 604, 7.2)
    canvas.checkbox(335, 613, "Identity", small=True)
    canvas.checkbox(407, 613, "Address", small=True)
    canvas.checkbox(481, 613, "Receipt", small=True)
    canvas.checkbox(335, 635, "Other", small=True)
    canvas.line(378, 646, 547, 646, dash="[1.2 2.4] 0")

    canvas.section(664, "D. DECLARATION AND SIGNATURE")
    canvas.text(
        "I confirm that the information on this form is complete and "
        "accurate. I understand that missing",
        48,
        696,
    )
    canvas.text(
        "information may delay the request.",
        48,
        708,
    )
    canvas.text(
        "Consent to be contacted about this request (circle one):  "
        "I AGREE   I DO NOT AGREE",
        48,
        727,
    )
    canvas.field("Signature", 48, 755, 98, 292)
    canvas.field("Date", 315, 755, 341, 432)
    canvas.field("Initials", 455, 755, 496, 547)

    canvas.rect(40, 775, 515, 27, line_width=0.55, gray=0.5)
    canvas.text("OFFICE DECISION", 48, 792, bold=True)
    canvas.text(
        "Circle one:  ACCEPTED   MORE INFORMATION REQUIRED   DECLINED",
        142,
        792,
        7.2,
    )
    canvas.text("Staff:", 449, 792, 7.2)
    canvas.line(482, 794, 547, 794)


def build_pdf(content: bytes) -> bytes:
    objects = [
        b"<< /Type /Catalog /Pages 2 0 R >>",
        b"<< /Type /Pages /Count 1 /Kids [3 0 R] >>",
        (
            b"<< /Type /Page /Parent 2 0 R "
            b"/MediaBox [0 0 595.28 841.89] "
            b"/Resources << /Font << /F1 4 0 R /F2 5 0 R >> >> "
            b"/Contents 6 0 R >>"
        ),
        (
            b"<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica "
            b"/Encoding /WinAnsiEncoding >>"
        ),
        (
            b"<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica-Bold "
            b"/Encoding /WinAnsiEncoding >>"
        ),
        b"<< /Length "
        + str(len(content)).encode("ascii")
        + b" >>\nstream\n"
        + content
        + b"endstream",
        (
            b"<< /Title (General service request form) "
            b"/Creator (FastFill) /Producer (FastFill) "
            b"/CreationDate (D:20260101000000Z) >>"
        ),
    ]

    pdf = bytearray(b"%PDF-1.4\n%\xe2\xe3\xcf\xd3\n")
    offsets = [0]
    for index, obj in enumerate(objects, start=1):
        offsets.append(len(pdf))
        pdf.extend(f"{index} 0 obj\n".encode("ascii"))
        pdf.extend(obj)
        pdf.extend(b"\nendobj\n")

    xref = len(pdf)
    pdf.extend(f"xref\n0 {len(objects) + 1}\n".encode("ascii"))
    pdf.extend(b"0000000000 65535 f \n")
    for offset in offsets[1:]:
        pdf.extend(f"{offset:010d} 00000 n \n".encode("ascii"))
    pdf.extend(
        (
            f"trailer\n<< /Size {len(objects) + 1} /Root 1 0 R "
            f"/Info 7 0 R >>\nstartxref\n{xref}\n%%EOF\n"
        ).encode("ascii")
    )
    return bytes(pdf)


def validate_pdf(pdf: bytes) -> None:
    required = (
        b"%PDF-1.4",
        b"/Type /Page ",
        b"/BaseFont /Helvetica ",
        b"(GENERAL SERVICE REQUEST FORM)",
        b"xref\n",
        b"%%EOF\n",
    )
    for marker in required:
        if marker not in pdf:
            raise ValueError(f"Generated PDF lacks {marker!r}.")


def main(args: list[str]) -> int:
    if len(args) != 2 or not args[1].strip():
        print(
            "Usage: generate_sample_form.py OUTPUT.pdf",
            file=sys.stderr,
        )
        return 2

    output = Path(args[1])
    if output.suffix.lower() != ".pdf":
        print("Output path must end in .pdf.", file=sys.stderr)
        return 2

    canvas = PdfCanvas()
    draw_form(canvas)
    pdf = build_pdf(canvas.content())
    validate_pdf(pdf)
    output.parent.mkdir(parents=True, exist_ok=True)
    output.write_bytes(pdf)
    print(f"Created {output}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main(sys.argv))
