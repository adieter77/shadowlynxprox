package cmd

import (
	"context"
	"fmt"
	"time"

	grpcclient "github.com/shadowlynx/prox-cli/pkg/grpc"
	"github.com/spf13/cobra"
)

var versionCmd = &cobra.Command{
	Use:   "version",
	Short: "Show version information",
	Long:  "Displays version information for the CLI and connected orchestrator.",
	Run:   runVersion,
}

func init() {
	rootCmd.AddCommand(versionCmd)
}

func runVersion(cmd *cobra.Command, args []string) {
	fmt.Printf("Shadowlynx ProX CLI\n")
	fmt.Printf("  Version: %s\n", Version)
	fmt.Printf("  Build:   development\n\n")

	// Try to get orchestrator version
	client, err := grpcclient.NewClient(grpcclient.ClientConfig{
		Address: orchestratorAddr,
	})
	if err != nil {
		fmt.Printf("Orchestrator: not connected (%v)\n", err)
		return
	}
	defer client.Close()

	ctx, cancel := context.WithTimeout(context.Background(), 5*time.Second)
	defer cancel()

	info, err := client.GetInfo(ctx)
	if err != nil {
		fmt.Printf("Orchestrator: error getting info (%v)\n", err)
		return
	}

	fmt.Printf("Orchestrator:\n")
	fmt.Printf("  Version: %s\n", info.Version)
	fmt.Printf("  Build:   %s (%s)\n", info.BuildCommit, info.BuildDate)
	fmt.Printf("  Models:  %v\n", info.AvailableModels)
}
