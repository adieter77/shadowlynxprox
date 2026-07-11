package cmd

import (
	"fmt"
	"os"

	"github.com/spf13/cobra"
)

var (
	// Version is set at build time with ldflags
	Version = "0.1.0"
	// These are populated from config
	orchestratorAddr string
	verbose          bool
)

// rootCmd is the base command. Everything else hangs off this.
var rootCmd = &cobra.Command{
	Use:   "slpx",
	Short: "Shadowlynx ProX - AI-powered terminal agent",
	Long: `Shadowlynx ProX is a terminal-based AI agent for cybersecurity,
software engineering, and blockchain operations.

It understands natural language, executes commands, generates
payloads, and performs deep analysis — all from your terminal.`,
	// This runs when you type 'slpx' with no subcommand
	Run: func(cmd *cobra.Command, args []string) {
		// If no subcommand given, show help
		if len(args) == 0 {
			cmd.Help()
			return
		}
	},
}

// Execute runs the root command
func Execute() error {
	return rootCmd.Execute()
}

func init() {
	// Persistent flags are available to all subcommands
	rootCmd.PersistentFlags().StringVarP(
		&orchestratorAddr,
		"server", "s",
		"localhost:50052",
		"Orchestrator gRPC address",
	)
	rootCmd.PersistentFlags().BoolVarP(
		&verbose,
		"verbose", "v",
		false,
		"Enable verbose output",
	)
}

// requireOrchestrator checks if the server flag was set (for commands that need it)
func requireOrchestrator(cmd *cobra.Command, args []string) error {
	if orchestratorAddr == "" {
		return fmt.Errorf("--server flag is required (e.g., --server localhost:50052)")
	}
	return nil
}

// exitWithError prints an error and exits
func exitWithError(err error) {
	fmt.Fprintf(os.Stderr, "Error: %v\n", err)
	os.Exit(1)
}
