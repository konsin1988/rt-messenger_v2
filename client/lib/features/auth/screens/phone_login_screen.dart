import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:go_router/go_router.dart';
import '../providers/auth_provider.dart';

class PhoneLoginScreen extends ConsumerStatefulWidget {
  const PhoneLoginScreen({super.key});

  @override
  ConsumerState<PhoneLoginScreen> createState() => _PhoneLoginScreenState();
}

class _PhoneLoginScreenState extends ConsumerState<PhoneLoginScreen> {
  final _phoneController = TextEditingController();
  final _formKey = GlobalKey<FormState>();
  bool _isLoading = false;

  @override
  void dispose() {
    _phoneController.dispose();
    super.dispose();
  }

  Future<void> _requestOtp() async {
    if (!_formKey.currentState!.validate()) return;
    setState(() => _isLoading = true);
    final phone = _phoneController.text.trim();
    try {
      final debugOtp = await ref.read(authProvider.notifier).requestOtp(phone);
      if (!mounted) return;
      if (debugOtp != null) {
        ScaffoldMessenger.of(context).showSnackBar(
          SnackBar(content: Text('Debug OTP: $debugOtp (SMS_MOCK)')),
        );
      }
      context.push('/otp', extra: phone);
    } catch (e) {
      if (!mounted) return;
      ScaffoldMessenger.of(context).showSnackBar(
        SnackBar(content: Text('Request failed: $e')),
      );
    } finally {
      if (mounted) setState(() => _isLoading = false);
    }
  }

  @override
  Widget build(BuildContext context) {
    return Scaffold(
      body: SafeArea(
        child: Padding(
          padding: const EdgeInsets.all(24),
          child: Form(
            key: _formKey,
            child: Column(
              mainAxisAlignment: MainAxisAlignment.center,
              crossAxisAlignment: CrossAxisAlignment.stretch,
              children: [
                const Icon(Icons.chat_bubble_rounded, size: 80, color: Color(0xFF6366F1)),
                const SizedBox(height: 16),
                Text('Messenger', style: Theme.of(context).textTheme.headlineMedium, textAlign: TextAlign.center),
                const SizedBox(height: 8),
                Text('Enter your phone to receive code', style: Theme.of(context).textTheme.bodyMedium, textAlign: TextAlign.center),
                const SizedBox(height: 32),
                TextFormField(
                  controller: _phoneController,
                  keyboardType: TextInputType.phone,
                  decoration: const InputDecoration(
                    hintText: '+79990001122',
                    prefixIcon: Icon(Icons.phone_outlined),
                    labelText: 'Phone',
                  ),
                  validator: (value) {
                    if (value == null || value.isEmpty) return 'Enter phone';
                    final re = RegExp(r'^\+?[0-9]{10,15}$');
                    if (!re.hasMatch(value.trim())) return '10-15 digits, optional +';
                    return null;
                  },
                ),
                const SizedBox(height: 24),
                ElevatedButton(
                  onPressed: _isLoading ? null : _requestOtp,
                  child: _isLoading
                      ? const SizedBox(height: 20, width: 20, child: CircularProgressIndicator(strokeWidth: 2))
                      : const Text('Get code'),
                ),
                const SizedBox(height: 16),
                TextButton(
                  onPressed: () => context.push('/login'),
                  child: const Text('Legacy email login'),
                ),
              ],
            ),
          ),
        ),
      ),
    );
  }
}
