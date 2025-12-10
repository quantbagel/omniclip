import 'package:flutter/material.dart';

/// Extension on BuildContext for showing snackbars.
extension SnackBarHelper on BuildContext {
  /// Show a success message snackbar.
  void showSuccess(String message) {
    if (!mounted) return;
    ScaffoldMessenger.of(this).showSnackBar(
      SnackBar(
        content: Text(message),
        backgroundColor: Colors.green,
        behavior: SnackBarBehavior.floating,
      ),
    );
  }

  /// Show an error message snackbar.
  void showError(String message) {
    if (!mounted) return;
    ScaffoldMessenger.of(this).showSnackBar(
      SnackBar(
        content: Text(message),
        backgroundColor: Colors.red,
        behavior: SnackBarBehavior.floating,
      ),
    );
  }

  /// Show an info message snackbar.
  void showInfo(String message) {
    if (!mounted) return;
    ScaffoldMessenger.of(this).showSnackBar(
      SnackBar(
        content: Text(message),
        behavior: SnackBarBehavior.floating,
      ),
    );
  }

  /// Hide any current snackbar.
  void hideSnackBar() {
    if (!mounted) return;
    ScaffoldMessenger.of(this).hideCurrentSnackBar();
  }

  /// Check if the context is still mounted (for async operations).
  bool get mounted {
    try {
      // This will throw if context is invalid
      findRenderObject();
      return true;
    } catch (_) {
      return false;
    }
  }
}

/// Run an async operation only if the context is still mounted.
/// Returns null if the context was unmounted.
Future<T?> runIfMounted<T>(
  BuildContext context,
  Future<T> Function() operation,
) async {
  try {
    final result = await operation();
    if (context.mounted) {
      return result;
    }
  } catch (_) {
    // Context was unmounted or operation failed
  }
  return null;
}

/// Protocol constants matching the Rust implementation.
class ProtocolConstants {
  static const int defaultPort = 17394;
  static const String serviceType = '_omniclip._tcp.local.';
  static const String pairingUrlScheme = 'omniclip://pair';
  static const String sessionKeyInfo = 'omniclip-session-key';
  static const int maxMessageSize = 10 * 1024 * 1024;
  static const int protocolVersion = 1;
}
