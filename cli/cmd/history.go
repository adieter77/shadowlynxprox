package cmd

import (
	"context"
	"encoding/json"
	"fmt"
	"os"
	"time"

	pb "github.com/shadowlynx/prox-cli/pkg/grpc/proto"
	"github.com/spf13/cobra"
)

var historyCmd = &cobra.Command{
	Use:   "history",
	Short: "View conversation history",
	Long:  `Display session info and recent chat history stored in Redis.`,
	Run: func(cmd *cobra.Command, args []string) {
		clear, _ := cmd.Flags().GetBool("clear")
		if clear {
			fmt.Println("Note: history --clear requires the AI Core's gRPC to be online.")
			return
		}

		count, _ := cmd.Flags().GetInt("count")
		format, _ := cmd.Flags().GetString("format")

		client, conn, err := getClient()
		if err != nil {
			fmt.Fprintf(os.Stderr, "Failed to connect to orchestrator: %v\n", err)
			fmt.Fprintf(os.Stderr, "Make sure the orchestrator is running: cd orchestrator && cargo run\n")
			os.Exit(1)
		}
		defer conn.Close()

		ctx, cancel := context.WithTimeout(context.Background(), 5*time.Second)
		defer cancel()

		resp, err := client.GetInfo(ctx, &pb.GetInfoRequest{})
		if err != nil {
			fmt.Fprintf(os.Stderr, "Error: %v\n", err)
			os.Exit(1)
		}

		if format == "json" {
			info := map[string]interface{}{
				"message_count": count,
				"version":       resp.Version,
				"build_commit":  resp.BuildCommit,
				"build_date":    resp.BuildDate,
				"models":        resp.AvailableModels,
				"plugins":       resp.AvailablePlugins,
			}
			json.NewEncoder(os.Stdout).Encode(info)
		} else {
			fmt.Println("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━")
			fmt.Println("  Shadowlynx ProX — Session")
			fmt.Println("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━")
			fmt.Printf("  Version:  %s\n", resp.Version)
			fmt.Printf("  Build:    %s\n", resp.BuildCommit)
			fmt.Printf("  Built:    %s\n", resp.BuildDate)
			fmt.Printf("  Max ctx:  %d tokens\n", resp.MaxContextTokens)
			fmt.Println("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━")
			if len(resp.AvailableModels) > 0 {
				fmt.Println("  Available models:")
				for _, m := range resp.AvailableModels {
					fmt.Printf("    • %s\n", m)
				}
			}
			if len(resp.AvailablePlugins) > 0 {
				fmt.Println("  Available plugins:")
				for _, p := range resp.AvailablePlugins {
					fmt.Printf("    • %s\n", p)
				}
			}
		}
	},
}

func init() {
	historyCmd.Flags().IntP("count", "n", 20, "Number of messages to show")
	historyCmd.Flags().Bool("clear", false, "Clear conversation history")
	historyCmd.Flags().String("format", "pretty", "Output format: pretty or json")
	rootCmd.AddCommand(historyCmd)
}
