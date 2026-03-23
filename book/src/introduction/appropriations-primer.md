# How Federal Appropriations Work

This chapter covers the essentials of federal appropriations — fiscal years, bill types, provision structure, and key terminology. Readers already familiar with the appropriations process can skip to the [tutorials](../tutorials/find-spending-on-topic.md).

## The Federal Budget in 60 Seconds

The U.S. federal government spends roughly **$6.7 trillion** per year. That breaks down into three major categories:

| Category | Share | What It Covers |
|----------|-------|----------------|
| **Mandatory spending** | ~63% | Social Security, Medicare, Medicaid, SNAP, and other programs where spending is determined by eligibility rules set in permanent law — not annual votes |
| **Discretionary spending** | ~26% | Everything Congress votes on each year through appropriations bills: defense, veterans' health care, scientific research, federal law enforcement, national parks, foreign aid, and thousands of other programs |
| **Net interest** | ~11% | Interest payments on the national debt |

**This tool covers the 26% — discretionary spending** — plus certain mandatory spending lines that appear as appropriation provisions in the bill text (for example, SNAP funding appears as a line item in the Agriculture appropriations division even though it's technically mandatory spending). That's why the budget authority total for H.R. 4366 is ~$846 billion, not the ~$1.7 trillion figure you'll sometimes see for total discretionary spending (which includes all twelve bills plus defense), and certainly not the ~$6.7 trillion total federal budget.

## The Fiscal Year

The federal fiscal year runs from **October 1 through September 30**. It's named for the calendar year in which it *ends*, not the one in which it begins. So:

- **FY2024** = October 1, 2023 – September 30, 2024
- **FY2025** = October 1, 2024 – September 30, 2025

Bills are labeled by the fiscal year they fund, not the calendar year they were enacted in. The Consolidated Appropriations Act, **2024** (H.R. 4366) was signed into law on March 23, **2024** — nearly six months into the fiscal year it was supposed to fund from the start.

## The Twelve Appropriations Bills

Each year, Congress is supposed to pass twelve individual appropriations bills, one for each subcommittee of the House and Senate Appropriations Committees:

1. Agriculture, Rural Development, FDA
2. Commerce, Justice, Science (CJS)
3. Defense
4. Energy and Water Development
5. Financial Services and General Government
6. Homeland Security
7. Interior, Environment
8. Labor, Health and Human Services, Education (Labor-HHS)
9. Legislative Branch
10. Military Construction, Veterans Affairs (MilCon-VA)
11. State, Foreign Operations
12. Transportation, Housing and Urban Development (THUD)

In practice, Congress rarely passes all twelve on time. Instead, it bundles them:

- An **omnibus** packages all (or nearly all) twelve bills into a single piece of legislation.
- A **minibus** bundles a few of the twelve together.
- Individual bills are occasionally passed on their own, but this has become increasingly rare.

When none of the twelve are done by October 1, Congress passes a **continuing resolution** to keep the government funded temporarily while it finishes negotiations.

## Bill Types

The included dataset covers 32 enacted appropriations bills spanning all major bill types. Here's what each one is, with the real example from this tool:

### Regular / Omnibus

A regular appropriations bill provides new funding for one of the twelve subcommittee jurisdictions for the coming fiscal year. An **omnibus** combines multiple regular bills into one legislative vehicle, organized into lettered divisions (Division A, Division B, etc.). **H.R. 4366**, the Consolidated Appropriations Act, 2024, is an omnibus covering MilCon-VA, Agriculture, CJS, Energy-Water, Interior, THUD, and other matters across multiple divisions. It contains **2,364 provisions** and authorizes **$846 billion** in budget authority.

### Continuing Resolution

A **continuing resolution (CR)** provides temporary funding — usually at the prior fiscal year's rate — for agencies whose regular appropriations bills haven't been enacted yet. Most provisions in a CR simply say "continue at last year's level," but specific programs may get different treatment through **anomalies** (formally called CR substitutions). **H.R. 5860**, the Continuing Appropriations Act, 2024, contains **130 provisions** including **13 CR substitutions** — programs where Congress set a specific dollar amount rather than defaulting to the prior-year rate. It also includes mandatory spending extensions and other legislative riders.

### Supplemental

A **supplemental** appropriation provides additional funding outside the regular annual cycle, typically in response to emergencies — natural disasters, military operations, public health crises, or (in this case) an unexpected funding shortfall. **H.R. 9468**, the Veterans Benefits Continuity and Accountability Supplemental Appropriations Act, 2024, contains **7 provisions** providing **$2.9 billion** for VA Compensation and Pensions and Readjustment Benefits, plus reporting requirements and an Inspector General review.

### Rescissions

A rescission bill *cancels* previously enacted budget authority. Rescissions also appear as individual provisions within larger bills — H.R. 4366 includes $24.7 billion in rescissions alongside its new appropriations.

## Anatomy of a Provision

To see how bill text becomes structured data, let's walk through a real example from H.R. 9468. Here's what Congress wrote:

> For an additional amount for ''Compensation and Pensions'', $2,285,513,000, to remain available until expended.

And here is the structured JSON that `congress-approp` extracted from that sentence:

```json
{
  "provision_type": "appropriation",
  "agency": "Department of Veterans Affairs",
  "account_name": "Compensation and Pensions",
  "amount": {
    "value": { "kind": "specific", "dollars": 2285513000 },
    "semantics": "new_budget_authority",
    "text_as_written": "$2,285,513,000"
  },
  "detail_level": "top_level",
  "availability": "to remain available until expended",
  "fiscal_year": 2024,
  "raw_text": "For an additional amount for ''Compensation and Pensions'', $2,285,513,000, to remain available until expended.",
  "confidence": 0.99
}
```

Here's what each piece means:

- **`account_name`**: Pulled from the double-quoted name in the bill text (the `''Compensation and Pensions''` delimiters are a legislative drafting convention).
- **`amount`**: The dollar value is parsed to an integer (`2285513000`), the original text is preserved (`"$2,285,513,000"`), and the meaning is classified — this is `new_budget_authority`, meaning Congress is granting new spending authority, not referencing an existing amount.
- **`detail_level`**: This is a `top_level` appropriation — the full amount for the account, not a sub-allocation ("of which $X for Y").
- **`availability`**: Captured from the bill text. "To remain available until expended" means this is no-year money — the agency can spend it over multiple fiscal years, unlike annual funds that expire at the end of the fiscal year.
- **`raw_text`**: The original bill text, verified against the source XML.
- **Verification**: The string `$2,285,513,000` was found at character position 431 in the source XML. The `raw_text` is a byte-identical substring of the source starting at position 371.

## Key Concepts

### Budget Authority vs. Outlays

**Budget authority (BA)** is what Congress authorizes — the legal permission for agencies to enter into obligations (sign contracts, award grants, hire staff). **Outlays** are what the Treasury actually disburses. The two differ because agencies often obligate funds in one year but spend them over several years (especially for construction, procurement, and multi-year grants).

This tool reports **budget authority**, because that's what the bill text specifies. When you see "$846B" for H.R. 4366, that's the sum of `new_budget_authority` provisions at the `top_level` and `line_item` detail levels — what Congress authorized, not what agencies will spend this year.

### Sub-Allocations Are Not Additional Money

Many provisions include "of which" clauses: *"For the Office of Science, $8,220,000,000, of which $300,000,000 shall be for fusion energy research."* The $300 million is a **sub-allocation** — a directive about how to spend part of the $8.2 billion, not money on top of it. The tool captures sub-allocations at `detail_level: "sub_allocation"` and correctly excludes them from budget authority totals to avoid double-counting.

### Advance Appropriations

Sometimes Congress enacts budget authority in this year's bill but makes it available starting in the *next* fiscal year. These **advance appropriations** are included in the bill's budget authority total (because the bill does enact them) but are noted in the provision's `notes` field.

### Congress Numbers

Each Congress spans two calendar years. The **118th Congress** served from January 2023 through January 2025; the **119th Congress** runs from January 2025 through January 2027. Bills are identified by their Congress — H.R. 4366 of the 118th Congress is an entirely different bill from H.R. 4366 of any other Congress. All three example bills in this tool are from the 118th Congress.

## Essential Glossary

These five terms come up throughout the book. A comprehensive glossary is available in the [Glossary reference chapter](../reference/glossary.md).

| Term | Definition |
|------|------------|
| **Budget authority** | The legal authority Congress grants to federal agencies to enter into financial obligations. This is the dollar figure in an appropriation provision — what Congress *authorizes*, as distinct from what agencies ultimately *spend* (outlays). |
| **Provision** | A single identifiable directive in an appropriations bill: an appropriation, a rescission, a spending limitation, a transfer authority, a CR anomaly, a policy rider, or any other discrete instruction. This is the fundamental unit of data in `congress-approp`. |
| **Enrolled** | The final text of a bill as passed by both the House and Senate and presented to the President for signature. This is the version `congress-approp` downloads — the authoritative text that becomes law. |
| **Rescission** | A provision that cancels previously enacted budget authority. A rescission of $500 million reduces the net budget authority by that amount. In the summary table, rescissions appear in their own column and are subtracted to produce the Net BA figure. |
| **Continuing resolution (CR)** | Temporary legislation that funds the government at the prior year's rate for agencies whose regular appropriations bills have not been enacted. Specific exceptions, called **anomalies** (or CR substitutions), set different funding levels for particular programs. |