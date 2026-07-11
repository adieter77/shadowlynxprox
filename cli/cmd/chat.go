package cmd

import (
	"bufio"
	"context"
	"fmt"
	"os"
	"strings"
	"time"

	pb "github.com/shadowlynx/prox-cli/pkg/grpc/orchestrator"
	grpcclient "github.com/shadowlynx/prox-cli/pkg/grpc"
	"github.com/spf13/cobra"
)

var chatCmd = &cobra.Command{
	Use:   "chat",
	Short: "Start interactive chat with Shadowlynx ProX",
	Long: `Launches an interactive REPL (Read-Eval-Print Loop) session
where you can talk to Shadowlynx ProX in natural language.

Type your questions or commands and the AI will respond.
Type 'exit' or 'quit' to leave, 'clear' to clear the screen.`,
	Run: runChat,
}

func init() {
	rootCmd.AddCommand(chatCmd)
}

func runChat(cmd *cobra.Command, args []string) {
	// Try to connect to orchestrator
	fmt.Println("╔══════════════════════════════════════════════════╗")
	fmt.Println("║           SHADOWLYNX ProX v0.1.0                  ║")
	fmt.Println("║           Terminal AI Agent                       ║")
	fmt.Println("╠══════════════════════════════════════════════════╣")

	// Attempt connection
	client, err := grpcclient.NewClient(grpcclient.ClientConfig{
		Address: orchestratorAddr,
	})
	if err != nil {
		fmt.Printf("║  ⚠ Orchestrator not available at %s ║\n", orchestratorAddr)
		fmt.Printf("║  Error: %s ║\n", truncateStr(err.Error(), 40))
		fmt.Println("╠══════════════════════════════════════════════════╣")
		fmt.Println("║  Starting in OFFLINE mode.                        ║")
		fmt.Println("║  Basic commands only.                             ║")
		fmt.Println("║  Start the orchestrator for full AI capabilities.  ║")
		fmt.Println("╚══════════════════════════════════════════════════╝")
		fmt.Println()
		runOfflineChat()
		return
	}
	defer client.Close()

	// Check health
	ctx, cancel := context.WithTimeout(context.Background(), 5*time.Second)
	defer cancel()

	health, err := client.HealthCheck(ctx)
	if err != nil || !health.Healthy {
		fmt.Println("║  ⚠ Orchestrator is not healthy                  ║")
		fmt.Println("╚══════════════════════════════════════════════════╝")
		fmt.Println()
		runOfflineChat()
		return
	}

	// Get orchestrator info
	info, err := client.GetInfo(ctx)
	if err == nil {
		fmt.Printf("║  Connected: %s (v%s) ║\n",
			orchestratorAddr, info.Version)
		fmt.Printf("║  Models: %s ║\n",
			truncateStr(strings.Join(info.AvailableModels, ", "), 35))
	} else {
		fmt.Printf("║  Connected: %s                        ║\n", orchestratorAddr)
	}
	fmt.Println("╠══════════════════════════════════════════════════╣")
	fmt.Println("║  Type your message and press Enter.               ║")
	fmt.Println("║  Type 'exit' to quit, 'clear' to clear screen.    ║")
	fmt.Println("║  Type 'help' for commands.                        ║")
	fmt.Println("╚══════════════════════════════════════════════════╝")
	fmt.Println()

	runOnlineChat(client)
}

func runOnlineChat(client *grpcclient.Client) {
	reader := bufio.NewReader(os.Stdin)
	var conversationID string

	for {
		fmt.Print("slpx> ")

		input, err := reader.ReadString('\n')
		if err != nil {
			fmt.Fprintf(os.Stderr, "\nError reading input: %v\n", err)
			break
		}

		input = strings.TrimSpace(input)
		if input == "" {
			continue
		}

		switch strings.ToLower(input) {
		case "exit", "quit", "q":
			fmt.Println("Goodbye, Shadowlynx.")
			return
		case "clear", "cls":
			fmt.Print("\033[2J\033[H")
			continue
		case "help":
			printHelp()
			continue
		case "info":
			ctx, cancel := context.WithTimeout(context.Background(), 5*time.Second)
			info, err := client.GetInfo(ctx)
			cancel()
			if err != nil {
				fmt.Printf("\n  Error: %v\n\n", err)
			} else {
				fmt.Printf("\n  Version: %s\n", info.Version)
				fmt.Printf("  Build: %s (%s)\n", info.BuildCommit, info.BuildDate)
				fmt.Printf("  Models: %s\n", strings.Join(info.AvailableModels, ", "))
				fmt.Printf("  Plugins: %s\n", strings.Join(info.AvailablePlugins, ", "))
				fmt.Println()
			}
			continue
		}

		// Send to orchestrator
		fmt.Println()
		ctx, cancel := context.WithTimeout(context.Background(), 120*time.Second)

		req := &pb.ChatRequest{
			ConversationId: conversationID,
			Message:        input,
		}

		err = client.Chat(ctx, req, func(chunk *pb.ChatResponse) error {
			// Print each chunk as it arrives (streaming)
			fmt.Print(chunk.TextChunk)

			// Save conversation ID from first response
			if chunk.ConversationId != "" {
				conversationID = chunk.ConversationId
			}
			return nil
		})
		cancel()

		if err != nil {
			fmt.Printf("\n  Error: %v\n", err)
		}
		fmt.Println()
		fmt.Println()
	}
}

func runOfflineChat() {
	reader := bufio.NewReader(os.Stdin)

	for {
		fmt.Print("slpx [offline]> ")

		input, err := reader.ReadString('\n')
		if err != nil {
			break
		}

		input = strings.TrimSpace(input)
		if input == "" {
			continue
		}

		switch strings.ToLower(input) {
		case "exit", "quit", "q":
			fmt.Println("Goodbye.")
			return
		case "clear", "cls":
			fmt.Print("\033[2J\033[H")
			continue
		case "help":
			printHelp()
			continue
		}

		// Simple offline responses
		fmt.Println()
		fmt.Printf("  [Offline mode] I received: %q\n", input)
		fmt.Println("  Start the orchestrator for full AI capabilities.")
		fmt.Println("  Run: cd orchestrator && cargo run")
		fmt.Println()
	}
}

func truncateStr(s string, maxLen int) string {
	if len(s) <= maxLen {
		return s
	}
	return s[:maxLen-3] + "..."
}

func printHelp() {
	fmt.Println()
	fmt.Println("Available commands (in-chat):")
	fmt.Println("  exit, quit, q    Exit the session")
	fmt.Println("  clear, cls       Clear the screen")
	fmt.Println("  help             Show this help")
	fmt.Println("  info             Show orchestrator information")
	fmt.Println()
	fmt.Println("You can type anything in natural language:")
	fmt.Println("  'scan example.com for open ports'")
	fmt.Println("  'generate a Python reverse shell'")
	fmt.Println("  'audit contracts/token.sol'")
	fmt.Println("  'build a REST API in Go'")
	fmt.Println("  'analyze this binary file'")
	fmt.Println()
}
