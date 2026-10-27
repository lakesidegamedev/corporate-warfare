#!/usr/bin/env python3
"""Check the authored design inputs and independent worked arithmetic, not a game test."""
import json
from fractions import Fraction
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
INDUSTRIES = {
    'agriculture', 'extraction', 'energy', 'chemical_industries', 'manufacturing',
    'construction', 'logistics', 'retail', 'healthcare', 'financial', 'technology',
    'media', 'telecommunications', 'real_estate', 'defense'
}


def check():
    data = json.loads((ROOT / 'content/initial-balance.json').read_text())
    products = {p['id']: p for p in data['products']}
    # 34 products and contracts at the original calibration, plus the nine goods of the 1.7 chains.
    assert len(products) == len(data['products']) == 43, '43 unique products/contracts required'
    kinds = {'goods', 'local_service', 'project_service', 'transport_service',
             'consumer_goods', 'subscription', 'service', 'lease_service',
             'contract_goods', 'financial_contract'}
    for p in products.values():
        assert p['kind'] in kinds, p['id']
        assert p['reference_price_cents'] >= 0, p['id']
        assert (p['reference_price_cents'] == 0) == (p['kind'] == 'financial_contract'), p['id']
    covered = {r['industry'] for r in data['recipes']} | {'financial'}
    assert covered == INDUSTRIES, 'industry coverage mismatch'
    assert data['financial_businesses'] == ['bank', 'property_insurer']
    assert len({r['id'] for r in data['recipes']}) == len(data['recipes']), 'duplicate recipe'
    for row in data['recipes']:
        assert row['output'] in products
        assert products[row['output']]['kind'] != 'financial_contract'
        assert all(x in products and q > 0 for x, q in row['weekly_inputs_lots'].items())
        assert row['weekly_output_lots'] > 0 and row['capex_cents'] > 0
        for by in row.get('by_products', []):
            assert by['product'] in products and by['weekly_lots'] > 0, row['id']
            assert by['product'] != row['output'], row['id']
            assert products[by['product']]['kind'] != 'financial_contract', row['id']
        assert row['build_weeks'] > 0 and row['life_years'] > 0
        assert len(row['workers_low_mid_high']) == 3 and all(x >= 0 for x in row['workers_low_mid_high'])
    farm = next(r for r in data['recipes'] if r['id'] == 'farm')
    revenue = farm['weekly_output_lots'] * products[farm['output']]['reference_price_cents']
    inputs = sum(q * products[p]['reference_price_cents'] for p, q in farm['weekly_inputs_lots'].items())
    payroll = sum(n * w for n, w in zip(farm['workers_low_mid_high'], data['baseline_weekly_wage_cents']))
    maintenance = Fraction(farm['capex_cents'] * data['maintenance_annual_bps'], 10000 * 52)
    depreciation = Fraction(farm['capex_cents'], farm['life_years'] * 52)
    ebitda = revenue - inputs - payroll - maintenance - data['fixture_land_rent_cents_weekly']
    ebit = ebitda - depreciation
    # One farm plant: 129 * $100; fuel 4 * $300 + power 2 * $1,200; crew 8/2/1 after the 1.3.1
    # margin floor. Exact fractions, so EBIT rounds to 236154 here where the engine's rounded
    # components give 236153.
    assert revenue == 1290000 and inputs == 360000 and payroll == 620000
    assert data['fixture_land_rent_cents_weekly'] == 20000
    assert round(ebitda) == 274615 and round(ebit) == 236154, 'farm worked example drift'
    # Independent dividends/claims/credit examples used by future game tests.
    assert Fraction(6, 8) * 100 == 75  # treasury excluded from eligible votes/dividend
    assert Fraction(5, 10) <= Fraction(1, 2) < Fraction(6, 10)
    assert min(100000, max(0, 30000 - 5000)) == 25000  # insured loss, deductible, limit
    assert Fraction(7, 10) * Fraction(1, 10) + Fraction(1, 4) * Fraction(3, 10) + Fraction(1, 20) == Fraction(195, 1000)
    print('Design inputs valid: 43 products, 15 industries; worked arithmetic consistent. No game behaviour certified.')


if __name__ == '__main__':
    check()
