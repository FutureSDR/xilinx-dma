void __dmb() {
	asm ("dmb sy" : : : "memory");
}
