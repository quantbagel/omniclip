import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:provider/provider.dart';
import '../services/sync_service.dart';
import '../utils/ui_helpers.dart';
import 'scanner_screen.dart';

class HomeScreen extends StatelessWidget {
  const HomeScreen({super.key});

  @override
  Widget build(BuildContext context) {
    return Scaffold(
      appBar: AppBar(
        title: const Text('Omniclip'),
        centerTitle: true,
      ),
      body: Consumer<SyncService>(
        builder: (context, service, _) {
          return Padding(
            padding: const EdgeInsets.all(20),
            child: Column(
              children: [
                // Connection status card
                _buildStatusCard(context, service),
                const SizedBox(height: 20),

                // Paired devices section
                if (service.pairedDevices.isNotEmpty) ...[
                  const Align(
                    alignment: Alignment.centerLeft,
                    child: Text(
                      'Paired Devices',
                      style: TextStyle(
                        fontSize: 18,
                        fontWeight: FontWeight.bold,
                      ),
                    ),
                  ),
                  const SizedBox(height: 10),
                  Expanded(
                    child: ListView.builder(
                      itemCount: service.pairedDevices.length,
                      itemBuilder: (context, index) {
                        final device = service.pairedDevices[index];
                        final isDeviceConnected = service.isConnected &&
                            service.connectedDeviceId == device.id;
                        return Card(
                          color: isDeviceConnected
                              ? Colors.green.withValues(alpha: 0.15)
                              : null,
                          child: ListTile(
                            leading: Stack(
                              children: [
                                const Icon(Icons.computer),
                                if (isDeviceConnected)
                                  Positioned(
                                    right: 0,
                                    bottom: 0,
                                    child: Container(
                                      width: 10,
                                      height: 10,
                                      decoration: BoxDecoration(
                                        color: Colors.green,
                                        shape: BoxShape.circle,
                                        border: Border.all(
                                          color: Theme.of(context).cardColor,
                                          width: 2,
                                        ),
                                      ),
                                    ),
                                  ),
                              ],
                            ),
                            title: Row(
                              children: [
                                Text(device.name),
                                if (isDeviceConnected) ...[
                                  const SizedBox(width: 8),
                                  Container(
                                    padding: const EdgeInsets.symmetric(
                                      horizontal: 6,
                                      vertical: 2,
                                    ),
                                    decoration: BoxDecoration(
                                      color: Colors.green,
                                      borderRadius: BorderRadius.circular(4),
                                    ),
                                    child: const Text(
                                      'Connected',
                                      style: TextStyle(
                                        color: Colors.white,
                                        fontSize: 10,
                                        fontWeight: FontWeight.bold,
                                      ),
                                    ),
                                  ),
                                ],
                              ],
                            ),
                            subtitle: Text('${device.host}:${device.port}'),
                            trailing: IconButton(
                              icon: const Icon(Icons.delete_outline),
                              onPressed: () => _confirmDelete(context, service, device.id),
                            ),
                          ),
                        );
                      },
                    ),
                  ),
                ] else ...[
                  const Expanded(
                    child: Center(
                      child: Column(
                        mainAxisSize: MainAxisSize.min,
                        children: [
                          Icon(
                            Icons.devices,
                            size: 80,
                            color: Colors.grey,
                          ),
                          SizedBox(height: 16),
                          Text(
                            'No paired devices',
                            style: TextStyle(
                              fontSize: 18,
                              color: Colors.grey,
                            ),
                          ),
                          SizedBox(height: 8),
                          Text(
                            'Scan a QR code to connect',
                            style: TextStyle(color: Colors.grey),
                          ),
                        ],
                      ),
                    ),
                  ),
                ],

                // Test clipboard button (when connected)
                if (service.isConnected) ...[
                  const Divider(),
                  ElevatedButton.icon(
                    onPressed: () => _sendTestClipboard(context, service),
                    icon: const Icon(Icons.content_paste),
                    label: const Text('Send Test Clipboard'),
                  ),
                ],
              ],
            ),
          );
        },
      ),
      floatingActionButton: Column(
        mainAxisSize: MainAxisSize.min,
        children: [
          FloatingActionButton(
            heroTag: 'manual',
            onPressed: () => _showManualEntry(context),
            child: const Icon(Icons.edit),
          ),
          const SizedBox(height: 12),
          FloatingActionButton.extended(
            heroTag: 'scan',
            onPressed: () => _openScanner(context),
            icon: const Icon(Icons.qr_code_scanner),
            label: const Text('Scan QR'),
          ),
        ],
      ),
    );
  }

  void _showManualEntry(BuildContext context) {
    final controller = TextEditingController();
    showDialog(
      context: context,
      builder: (context) => AlertDialog(
        title: const Text('Enter Pairing URL'),
        content: TextField(
          controller: controller,
          decoration: const InputDecoration(
            hintText: 'omniclip://pair?...',
            border: OutlineInputBorder(),
          ),
          maxLines: 3,
          style: const TextStyle(fontSize: 12),
        ),
        actions: [
          TextButton(
            onPressed: () => Navigator.pop(context),
            child: const Text('Cancel'),
          ),
          TextButton(
            onPressed: () async {
              Navigator.pop(context);
              await _connectWithUrl(context, controller.text.trim());
            },
            child: const Text('Connect'),
          ),
        ],
      ),
    );
  }

  Future<void> _connectWithUrl(BuildContext context, String url) async {
    if (url.isEmpty) return;

    final service = context.read<SyncService>();
    final pairingData = service.parsePairingUrl(url);

    if (pairingData == null) {
      if (context.mounted) {
        context.showError('Invalid pairing URL');
      }
      return;
    }

    // Show loading
    if (context.mounted) {
      context.showInfo('Connecting to ${pairingData.deviceName}...');
    }

    final success = await service.connectToPeer(pairingData);

    if (context.mounted) {
      context.hideSnackBar();
      if (success) {
        context.showSuccess('Connected to ${pairingData.deviceName}!');
      } else {
        context.showError('Connection failed');
      }
    }
  }

  Widget _buildStatusCard(BuildContext context, SyncService service) {
    final isConnected = service.isConnected;
    final colorScheme = Theme.of(context).colorScheme;

    return Card(
      color: isConnected
          ? colorScheme.primaryContainer
          : colorScheme.surfaceContainerHighest,
      child: Padding(
        padding: const EdgeInsets.all(20),
        child: Row(
          children: [
            Container(
              width: 12,
              height: 12,
              decoration: BoxDecoration(
                shape: BoxShape.circle,
                color: isConnected ? Colors.green : Colors.grey,
              ),
            ),
            const SizedBox(width: 12),
            Expanded(
              child: Column(
                crossAxisAlignment: CrossAxisAlignment.start,
                children: [
                  Text(
                    isConnected ? 'Connected' : 'Not Connected',
                    style: const TextStyle(
                      fontWeight: FontWeight.bold,
                      fontSize: 16,
                    ),
                  ),
                  Text(
                    isConnected
                        ? 'Clipboard syncing active'
                        : 'Scan QR code to connect',
                    style: TextStyle(
                      color: colorScheme.onSurfaceVariant,
                    ),
                  ),
                ],
              ),
            ),
            if (isConnected)
              TextButton(
                onPressed: () => service.disconnect(),
                child: const Text('Disconnect'),
              ),
          ],
        ),
      ),
    );
  }

  void _openScanner(BuildContext context) {
    Navigator.push(
      context,
      MaterialPageRoute(builder: (_) => const ScannerScreen()),
    );
  }

  void _confirmDelete(BuildContext context, SyncService service, String deviceId) {
    showDialog(
      context: context,
      builder: (context) => AlertDialog(
        title: const Text('Remove Device'),
        content: const Text('Are you sure you want to remove this paired device?'),
        actions: [
          TextButton(
            onPressed: () => Navigator.pop(context),
            child: const Text('Cancel'),
          ),
          TextButton(
            onPressed: () {
              service.removePairedDevice(deviceId);
              Navigator.pop(context);
            },
            child: const Text('Remove'),
          ),
        ],
      ),
    );
  }

  Future<void> _sendTestClipboard(BuildContext context, SyncService service) async {
    final data = await Clipboard.getData(Clipboard.kTextPlain);
    if (data?.text != null && data!.text!.isNotEmpty) {
      await service.sendClipboard(data.text!);
      if (context.mounted) {
        context.showSuccess('Clipboard sent!');
      }
    } else {
      if (context.mounted) {
        context.showInfo('Clipboard is empty');
      }
    }
  }
}
