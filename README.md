# Lookup Table Registry

A Solana program that stores address lookup tables for easier reference and usage.

The program creates a lookup table registry that stores and manages the address lookup tables of an 'authority'.

This authority can be a PDA, however a program wishing to make PDAs sign should do this via CPI.

## Testing

Run the fast tests with `./check`.

To include localnet test, which takes five minutes to complete, run `./check full`.
