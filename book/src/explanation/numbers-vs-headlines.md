# Why the Numbers Might Not Match Headlines

If you run `congress-approp summary --dir data` and see the budget numbers, your first reaction might be: *"That doesn't match any number I've seen in the news."* Headlines about the federal budget typically cite figures like $6.7 trillion (total spending), $1.7 trillion (total discretionary), or sometimes $1.2 trillion or $886 billion (specific spending cap categories).

This chapter explains the three main reasons for the discrepancy — and why the tool's number is correct for what it measures.

## The Three Budget Numbers

There are at least three different "federal budget" numbers in common use, and they measure fundamentally different things:

| Number | What It Measures | Source |
|--------|-----------------|--------|
| **~$6.7 trillion** | Total federal spending (outlays) — mandatory + discretionary + interest | CBO, OMB, Treasury |
| **~$1.7 trillion** | Total discretionary budget authority — all 12 appropriations bills combined | CBO scoring of appropriations acts |
| **$846 billion** (this tool, H.R. 4366) | Budget authority enacted in one specific bill (7 of 12 appropriations bills, plus mandatory lines that appear in the text) | Computed from individual provisions |

None of these numbers are wrong — they just measure different things at different levels of aggregation.

## Reason 1: This Omnibus Doesn't Cover All 12 Bills

Congress is supposed to pass 12 annual appropriations bills, one for each subcommittee jurisdiction. In practice, they're often bundled into an omnibus or split across multiple legislative vehicles.

The FY2024 omnibus (H.R. 4366, the Consolidated Appropriations Act, 2024) covers these divisions:

| Division | Coverage |
|----------|----------|
| A | Military Construction, Veterans Affairs |
| B | Agriculture, Rural Development, FDA |
| C | Commerce, Justice, Science |
| D | Energy and Water Development |
| E | Interior, Environment |
| F | Transportation, Housing and Urban Development |
| G–H | Other matters |

It does **not** include:

- **Defense** (by far the largest single appropriations bill, ~$886 billion in the FY2024 NDAA)
- **Labor, HHS, Education** (typically the largest domestic bill)
- **Homeland Security**
- **State, Foreign Operations**
- **Financial Services and General Government**
- **Legislative Branch**

Those were addressed through other legislative vehicles for FY2024. Since the tool only extracts what's in the bills you give it, the $846 billion total reflects 7 of 12 subcommittee jurisdictions — not the full discretionary budget.

**To get the full picture:** Extract all enacted appropriations bills for a congress, then run `summary --dir data` across all of them.

## Reason 2: Mandatory Spending Appears in Appropriations Bills

Some of the largest federal programs — technically classified as "mandatory spending" — appear as appropriation line items in the bill text. The tool extracts what the bill says without distinguishing mandatory from discretionary.

Notable mandatory programs in the H.R. 4366 example data:

| Account | Amount | Technically... |
|---------|--------|---------------|
| Compensation and Pensions (VA) | $197,382,903,000 | Mandatory entitlement |
| Supplemental Nutrition Assistance Program (SNAP) | $122,382,521,000 | Mandatory entitlement |
| Child Nutrition Programs | $33,266,226,000 | Mostly mandatory |
| Readjustment Benefits (VA) | $13,774,657,000 | Mandatory entitlement |

These four programs alone account for over $366 billion — nearly half of the omnibus total. They're in the bill because Congress appropriates the funds even though the spending levels are determined by eligibility rules in permanent law (the authorizing statutes), not by the annual appropriations process.

**Why the tool includes them:** The tool faithfully extracts every provision in the bill text. A provision that says "For Compensation and Pensions, $197,382,903,000" is an appropriation provision regardless of whether budget analysts classify the underlying program as mandatory. Distinguishing mandatory from discretionary requires authorizing-law context beyond the bill itself — context the tool doesn't have.

**How to identify mandatory lines:** Look for very large amounts in Division A (VA) and Division B (Agriculture). Programs with amounts in the tens or hundreds of billions are almost certainly mandatory. The `notes` field sometimes flags these, and you can filter them using `--max-dollars` to exclude the largest accounts from analysis.

## Reason 3: Budget Authority vs. Outlays

The most fundamental distinction in federal budgeting:

- **Budget Authority (BA):** The legal authority Congress grants to agencies to enter into financial obligations — sign contracts, award grants, hire staff. This is what the bill text specifies and what this tool reports.

- **Outlays:** The actual cash disbursements by the U.S. Treasury. This is what the government actually spends in a given year.

Budget authority and outlays differ because agencies often obligate funds in one year but spend them over several years. A multi-year construction project might receive $500 million in budget authority in FY2024, but the Treasury only disburses $100 million in FY2024, $200 million in FY2025, and $200 million in FY2026.

**Headline federal spending numbers are in outlays.** When you read "the federal government spent $6.7 trillion in FY2024," that's outlays — actual cash out the door. This tool reports budget authority — the amount Congress authorized agencies to commit. The two numbers are related but not identical, and budget authority is typically lower than outlays in any given year because outlays include spending from prior years' budget authority.

| Concept | What It Measures | Reported By This Tool? |
|---------|-----------------|----------------------|
| Budget Authority (BA) | What Congress authorizes | **Yes** |
| Obligations | What agencies commit to spend | No |
| Outlays | What Treasury actually pays out | No |

**Why BA is the right measure for this tool:** Budget authority is what the bill text specifies. It's the number Congress votes on, the number the Appropriations Committee reports, and the number that determines whether spending caps are breached. It's the most precise measure of congressional intent — "how much did Congress decide to give this program?"

## Reason 4: Advance Appropriations

Some provisions enact budget authority in the current year's bill but make the funds available starting in a *future* fiscal year. These **advance appropriations** are common for VA medical accounts:

For example, H.R. 4366 includes both:
- $71 billion for VA Medical Services in FY2024 (current-year appropriation)
- Advance appropriation amounts for VA Medical Services in FY2025

Both are counted in the bill's budget authority total because both are enacted by this bill. But from a fiscal year perspective, the advance amounts will be "FY2025 spending" even though the legal authority was enacted in the FY2024 bill.

The tool captures advance appropriations and typically flags them in the `notes` field. CBO scores may attribute them to different fiscal years than this tool's simple per-bill sum.

## Reason 5: Gross vs. Net Budget Authority

The summary table shows both gross budget authority and rescissions separately:

```text
│ H.R. 4366 ┆ Omnibus ┆ 2364 ┆ 846,137,099,554 ┆ 24,659,349,709 ┆ 821,477,749,845 │
```

- **Budget Auth ($846.1B):** Gross new budget authority
- **Rescissions ($24.7B):** Previously appropriated funds being canceled
- **Net BA ($821.5B):** The actual net new spending authority

Some external sources report gross BA, some report net BA, and some report net BA after other adjustments (offsets, fees, etc.). Make sure you're comparing like to like.

## How to Reconcile with External Sources

### CBO cost estimates

The Congressional Budget Office publishes cost estimates for most appropriations bills. These are the gold standard for budget scoring. To compare:

1. Find the CBO cost estimate for the specific bill (e.g., H.R. 4366)
2. Look at the "discretionary" budget authority line
3. Note that CBO separates discretionary from mandatory — this tool does not
4. Note that CBO may attribute advance appropriations to different fiscal years

### Appropriations Committee reports

House and Senate Appropriations Committee reports contain detailed funding tables by account. These are useful for account-level verification:

1. Find the committee report for the bill's division (e.g., Division A report for MilCon-VA)
2. Compare individual account amounts — these should match exactly
3. Compare title-level or division-level subtotals

### OMB Budget Appendix

The Office of Management and Budget publishes the Budget Appendix with account-level detail. This is useful for cross-checking agency totals but uses a different fiscal year attribution than this tool.

## Summary: What This Tool's Numbers Mean

When you see a budget authority figure from this tool, it means:

1. **It's computed from individual provisions** — not from any summary or LLM estimate
2. **It includes both discretionary and mandatory** spending lines that appear in the bill text
3. **It covers only the bills you've loaded** — not necessarily all 12 appropriations bills
4. **It reports budget authority** — what Congress authorized, not what agencies will actually spend
5. **It may include advance appropriations** — funds enacted now but available in future fiscal years
6. **Sub-allocations are correctly excluded** — "of which" breakdowns don't double-count
7. **Every dollar amount was verified** against the source bill text (0 unverifiable amounts across example data)

The number is precisely what the bill says. Whether that matches a headline depends on which bill, which measure (BA vs. outlays), and which programs (discretionary only vs. including mandatory) the headline is reporting.

## Quick Reference: Common Discrepancy Sources

| Your Number Seems... | Likely Cause | How to Check |
|----------------------|-------------|-------------|
| Too high vs. "discretionary spending" | Mandatory spending lines (SNAP, VA Comp & Pensions) included | Filter with `--max-dollars 50000000000` to see without the largest accounts |
| Too low vs. "total federal budget" | BA ≠ outlays; not all 12 bills loaded | Check which divisions/bills are in your data |
| Different from CBO score | Advance appropriations, mandatory/discretionary split, net vs. gross | Compare specific accounts rather than totals |
| Doesn't match committee report | Sub-allocations excluded from BA total; different aggregation level | Use `search --account` for account-level comparison |

## Next Steps

- **[Budget Authority Calculation](./budget-authority.md)** — the exact formula and what's included/excluded
- **[How Federal Appropriations Work](../introduction/appropriations-primer.md)** — background on bill types and the budget process
- **[Verify Extraction Accuracy](../how-to/verify-accuracy.md)** — cross-checking with external sources