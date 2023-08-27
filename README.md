# satisfactory-planner ![Build Status](https://img.shields.io/github/actions/workflow/status/storance/satisfactory-planner/rust.yml?branch=main)
A simple command line tool for generating factory plans for the game [Satisfactory](https://www.satisfactorygame.com/).

This is similar other tools like [Satisfactory Tools](https://www.satisfactorytools.com/) and [Satisfactory-Calculator](https://satisfactory-calculator.com/).

## Plan Config
```yaml
enabled_recipes:
  - base
  - alt
  - Pure Iron Ingot
  - exclude: Iron Ingot
inputs:
  Screw: 56
  Crude Oil: 0
outputs: 
  Reinforced Iron Plate: 8
  Iron Plate: 60
  Iron Rod: 30
```