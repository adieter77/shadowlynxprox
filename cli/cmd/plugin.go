package cmd

import (
	"context"
	"encoding/json"
	"fmt"
	"os"
	"time"

	pb "github.com/shadowlynx/prox-cli/pkg/grpc/proto"
	"github.com/spf13/cobra"
	"google.golang.org/grpc"
	"google.golang.org/grpc/credentials/insecure"
)

var pluginCmd = &cobra.Command{
	Use:   "plugin",
	Short: "Manage Shadowlynx ProX plugins",
	Long: `Load, list, run, and unload WASM plugins.

Plugins extend the agent with custom tools written in WebAssembly.
Each plugin runs in a sandboxed environment with capability-based security.`,
}

var pluginListCmd = &cobra.Command{
	Use:   "list",
	Short: "List all loaded plugins",
	Run: func(cmd *cobra.Command, args []string) {
		client, conn, err := getPluginClient()
		if err != nil {
			fmt.Fprintf(os.Stderr, "Failed to connect to orchestrator: %v\n", err)
			fmt.Fprintf(os.Stderr, "Make sure the orchestrator is running on localhost:50052\n")
			os.Exit(1)
		}
		defer conn.Close()

		ctx, cancel := context.WithTimeout(context.Background(), 5*time.Second)
		defer cancel()

		resp, err := client.ListPlugins(ctx, &pb.ListPluginsRequest{})
		if err != nil {
			fmt.Fprintf(os.Stderr, "Error: %v\n", err)
			os.Exit(1)
		}

		if len(resp.Plugins) == 0 {
			fmt.Println("No plugins loaded.")
			fmt.Println("Use 'slpx plugin load <file.wasm>' to load a plugin.")
			return
		}

		for _, p := range resp.Plugins {
			fmt.Printf("━━━ %s v%s ━━━\n", p.Manifest.Name, p.Manifest.Version)
			fmt.Printf("  ID:          %s\n", p.PluginId)
			fmt.Printf("  Author:      %s\n", p.Manifest.Author)
			fmt.Printf("  Description: %s\n", p.Manifest.Description)
			if len(p.Tools) > 0 {
				fmt.Println("  Tools:")
				for _, t := range p.Tools {
					fmt.Printf("    • %s\n", t.Name)
				}
			}
			fmt.Println()
		}
	},
}

var pluginLoadCmd = &cobra.Command{
	Use:   "load [file.wasm]",
	Short: "Load a WASM plugin",
	Args:  cobra.ExactArgs(1),
	Run: func(cmd *cobra.Command, args []string) {
		wasmPath := args[0]
		wasmBytes, err := os.ReadFile(wasmPath)
		if err != nil {
			fmt.Fprintf(os.Stderr, "Failed to read plugin: %v\n", err)
			os.Exit(1)
		}

		client, conn, err := getPluginClient()
		if err != nil {
			fmt.Fprintf(os.Stderr, "Failed to connect: %v\n", err)
			os.Exit(1)
		}
		defer conn.Close()

		ctx, cancel := context.WithTimeout(context.Background(), 30*time.Second)
		defer cancel()

		// Validate first
		validateResp, err := client.ValidatePlugin(ctx, &pb.ValidatePluginRequest{
			WasmBytes: wasmBytes,
		})
		if err != nil {
			fmt.Fprintf(os.Stderr, "Validation RPC failed: %v\n", err)
			os.Exit(1)
		}
		if !validateResp.Valid {
			fmt.Fprintf(os.Stderr, "Invalid plugin: %s\n", validateResp.Error)
			os.Exit(1)
		}

		fmt.Printf("Validated: %s v%s (%d tools)\n",
			validateResp.Manifest.Name, validateResp.Manifest.Version, len(validateResp.Tools))

		// Load
		loadResp, err := client.LoadPlugin(ctx, &pb.LoadPluginRequest{
			Source:    &pb.LoadPluginRequest_WasmBytes{WasmBytes: wasmBytes},
			Manifest:  validateResp.Manifest,
		})
		if err != nil {
			fmt.Fprintf(os.Stderr, "Load RPC failed: %v\n", err)
			os.Exit(1)
		}
		if loadResp.Success {
			fmt.Printf("✓ Plugin loaded: %s\n", loadResp.PluginId)
		} else {
			fmt.Fprintf(os.Stderr, "✗ Load failed: %s\n", loadResp.Error)
			os.Exit(1)
		}
	},
}

var pluginRunCmd = &cobra.Command{
	Use:   "run [plugin-id] [tool-name] [args-json]",
	Short: "Run a plugin tool",
	Args:  cobra.ExactArgs(3),
	Run: func(cmd *cobra.Command, args []string) {
		pluginID := args[0]
		toolName := args[1]
		argsJSON := args[2]

		client, conn, err := getPluginClient()
		if err != nil {
			fmt.Fprintf(os.Stderr, "Failed to connect: %v\n", err)
			os.Exit(1)
		}
		defer conn.Close()

		ctx, cancel := context.WithTimeout(context.Background(), 120*time.Second)
		defer cancel()

		resp, err := client.ExecuteTool(ctx, &pb.ExecuteToolRequest{
			PluginId:      pluginID,
			ToolName:      toolName,
			ArgumentsJson: argsJSON,
		})
		if err != nil {
			fmt.Fprintf(os.Stderr, "Error: %v\n", err)
			os.Exit(1)
		}

		if resp.Result.Success {
			var prettyJSON interface{}
			if err := json.Unmarshal([]byte(resp.Result.Output), &prettyJSON); err == nil {
				formatted, _ := json.MarshalIndent(prettyJSON, "", "  ")
				fmt.Println(string(formatted))
			} else {
				fmt.Println(resp.Result.Output)
			}
			fmt.Printf("\nDuration: %dms\n", resp.Result.DurationMs)
		} else {
			fmt.Fprintf(os.Stderr, "Tool failed: %s\n", resp.Result.Error)
			os.Exit(1)
		}
	},
}

var pluginUnloadCmd = &cobra.Command{
	Use:   "unload [plugin-id]",
	Short: "Unload a plugin",
	Args:  cobra.ExactArgs(1),
	Run: func(cmd *cobra.Command, args []string) {
		pluginID := args[0]
		client, conn, err := getPluginClient()
		if err != nil {
			fmt.Fprintf(os.Stderr, "Failed to connect: %v\n", err)
			os.Exit(1)
		}
		defer conn.Close()

		ctx, cancel := context.WithTimeout(context.Background(), 5*time.Second)
		defer cancel()

		resp, err := client.UnloadPlugin(ctx, &pb.UnloadPluginRequest{PluginId: pluginID})
		if err != nil {
			fmt.Fprintf(os.Stderr, "Error: %v\n", err)
			os.Exit(1)
		}
		if resp.Success {
			fmt.Printf("✓ Plugin unloaded: %s\n", pluginID)
		} else {
			fmt.Fprintf(os.Stderr, "✗ Failed: %s\n", resp.Error)
			os.Exit(1)
		}
	},
}

var pluginValidateCmd = &cobra.Command{
	Use:   "validate [file.wasm]",
	Short: "Validate a WASM plugin without loading it",
	Args:  cobra.ExactArgs(1),
	Run: func(cmd *cobra.Command, args []string) {
		wasmPath := args[0]
		wasmBytes, err := os.ReadFile(wasmPath)
		if err != nil {
			fmt.Fprintf(os.Stderr, "Failed to read plugin: %v\n", err)
			os.Exit(1)
		}

		client, conn, err := getPluginClient()
		if err != nil {
			fmt.Fprintf(os.Stderr, "Failed to connect: %v\n", err)
			os.Exit(1)
		}
		defer conn.Close()

		ctx, cancel := context.WithTimeout(context.Background(), 10*time.Second)
		defer cancel()

		resp, err := client.ValidatePlugin(ctx, &pb.ValidatePluginRequest{WasmBytes: wasmBytes})
		if err != nil {
			fmt.Fprintf(os.Stderr, "Error: %v\n", err)
			os.Exit(1)
		}
		if resp.Valid {
			fmt.Printf("✓ Valid: %s v%s\n", resp.Manifest.Name, resp.Manifest.Version)
			fmt.Printf("  ID:    %s\n", resp.Manifest.Id)
			fmt.Printf("  Tools: %d\n", len(resp.Tools))
			for _, t := range resp.Tools {
				fmt.Printf("    • %s\n", t.Name)
			}
		} else {
			fmt.Fprintf(os.Stderr, "✗ Invalid: %s\n", resp.Error)
			os.Exit(1)
		}
	},
}

func getPluginClient() (pb.PluginServiceClient, *grpc.ClientConn, error) {
	conn, err := grpc.NewClient(
		"localhost:50052",
		grpc.WithTransportCredentials(insecure.NewCredentials()),
	)
	if err != nil {
		return nil, nil, err
	}
	return pb.NewPluginServiceClient(conn), conn, nil
}

func init() {
	pluginCmd.AddCommand(pluginListCmd)
	pluginCmd.AddCommand(pluginLoadCmd)
	pluginCmd.AddCommand(pluginRunCmd)
	pluginCmd.AddCommand(pluginUnloadCmd)
	pluginCmd.AddCommand(pluginValidateCmd)
	rootCmd.AddCommand(pluginCmd)
}
